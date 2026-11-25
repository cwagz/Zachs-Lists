use anyhow::{ensure, Result};
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use tracing::info;

use crate::cancellation::{Cancellation, WORK_GRAIN};
use crate::generation::{
    create_new_file, is_safe_list_name, CountingWriter, HashingWriter, ManifestFile,
};

pub const COMBINED_LIST_NAME: &str = "all_domains";
pub const UNCATEGORIZED_LIST_NAME: &str = "uncategorized";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GenerationEvent {
    FileStarted(OutputFormat),
    DomainsWritten(OutputFormat, u64),
    FileCompleted { format: OutputFormat, gz_size: u64 },
}

pub struct GenerationControl<'a> {
    pub cancellation: &'a Cancellation,
    pub report: &'a (dyn Fn(GenerationEvent) + Send + Sync),
}

pub fn category_list_names<'a>(
    categories: impl Iterator<Item = &'a Option<String>>,
) -> Result<Vec<&'a str>> {
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for category in categories {
        let name = category.as_deref().unwrap_or(UNCATEGORIZED_LIST_NAME);
        ensure!(
            is_safe_list_name(name) && name != COMBINED_LIST_NAME,
            "Category name {name:?} cannot be used as a list name"
        );
        ensure!(
            seen.insert(name),
            "Two categories would write the list {name:?}"
        );
        names.push(name);
    }
    Ok(names)
}

/// Output format types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Hosts,
    Plain,
    Adblock,
}

impl OutputFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Hosts => "hosts",
            OutputFormat::Plain => "plain",
            OutputFormat::Adblock => "adblock",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            OutputFormat::Hosts => 0,
            OutputFormat::Plain => 1,
            OutputFormat::Adblock => 2,
        }
    }

    pub fn comment_prefix(&self) -> &'static str {
        match self {
            OutputFormat::Hosts | OutputFormat::Plain => "#",
            OutputFormat::Adblock => "!",
        }
    }

    pub fn all() -> Vec<OutputFormat> {
        vec![
            OutputFormat::Hosts,
            OutputFormat::Plain,
            OutputFormat::Adblock,
        ]
    }
}

/// Adblock options that do not change a DNS block. A stored rule whose
/// options are all in this set is written as `||domain^`.
///
/// `$all` blocks every request type, which is what a DNS block already does.
/// `$important` only outranks exception rules in a browser. The whitelist has
/// already removed those domains, and Pi-hole v6 drops a line that keeps the
/// option. `$document` / `$doc` and `$popup` are browser request kinds.
/// `$third-party` / `$3p` limit the rule to request context a resolver cannot
/// see. The extractor still refuses `$third-party` before a domain is
/// admitted, so listing it here does not start blocking first-party traffic.
/// `$3p` is the same option and is not in that skip list; a domain it already
/// admitted stays blocked and the option is removed.
const STRIPPED_ADBLOCK_MODIFIERS: &[&str] = &[
    "3p",
    "all",
    "doc",
    "document",
    "important",
    "popup",
    "third-party",
];

fn adblock_option_is_stripped(option: &str) -> bool {
    let name = option
        .split_once('=')
        .map(|(name, _)| name)
        .unwrap_or(option)
        .trim();
    !name.is_empty()
        && !name.starts_with('~')
        && STRIPPED_ADBLOCK_MODIFIERS
            .iter()
            .any(|known| name.eq_ignore_ascii_case(known))
}

/// True when `raw` has no `$` options, or every option is in
/// [`STRIPPED_ADBLOCK_MODIFIERS`]. A negated option, or any other name,
/// is not neutral: copying it would be wrong, and so would inventing a
/// block the extractor had not already admitted.
fn raw_adblock_rule_is_dns_neutral(raw: &str) -> bool {
    match raw.split_once('$') {
        None => true,
        Some((_, options)) => {
            !options.is_empty() && options.split(',').all(adblock_option_is_stripped)
        }
    }
}

/// A stored rule is copied only when it has no `$` options. Neutral options
/// are stripped instead, and any other option is dropped with the raw text.
/// Both of those are written as `||domain^`.
fn raw_adblock_rule_can_be_copied(raw: &str) -> bool {
    raw_adblock_rule_is_dns_neutral(raw) && !raw.contains('$')
}

/// Output file generator
pub struct OutputGenerator {
    output_dir: PathBuf,
    generator_host: String,
}

impl OutputGenerator {
    /// Create a generator that stamps the shared fallback host.
    #[cfg(test)]
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self::with_host(output_dir, crate::generator_host::BLOCKLIST_GENERATOR_HOST)
    }

    /// Create a generator that stamps `generator_host` on every file.
    pub fn with_host(output_dir: impl Into<PathBuf>, generator_host: impl Into<String>) -> Self {
        Self {
            output_dir: output_dir.into(),
            generator_host: generator_host.into(),
        }
    }

    /// Generate header lines for output file
    fn generate_header(&self, format: OutputFormat, domain_count: u64) -> String {
        let prefix = format.comment_prefix();
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
        let host = &self.generator_host;

        format!(
            "{} Blocklist generated by {}\n\
             {} Total domains: {}\n\
             {} Generated: {}\n\n",
            prefix, host, prefix, domain_count, prefix, now
        )
    }

    /// Write one domain line, without building an intermediate String.
    ///
    /// Hosts and plain use the domain only. Adblock is `||domain^`, which is
    /// the block Pi-hole v6 and AdGuard Home both accept. A stored rule with
    /// no `$` is copied. Options in [`STRIPPED_ADBLOCK_MODIFIERS`] are
    /// stripped, and any other option is discarded with the raw rule. The
    /// domain stays a block, because the extractor already admitted it and
    /// hosts and plain already block it. Pi-hole ignores a line that still
    /// has `$`.
    #[inline]
    fn write_domain<W: Write, R: AsRef<str>>(
        encoder: &mut W,
        format: OutputFormat,
        domain: &str,
        adblock_rules: Option<&HashMap<String, R>>,
    ) -> std::io::Result<()> {
        match format {
            OutputFormat::Hosts => {
                encoder.write_all(b"0.0.0.0 ")?;
                encoder.write_all(domain.as_bytes())?;
                encoder.write_all(b"\n")?;
            }
            OutputFormat::Plain => {
                encoder.write_all(domain.as_bytes())?;
                encoder.write_all(b"\n")?;
            }
            OutputFormat::Adblock => {
                if let Some(raw_rule) = adblock_rules
                    .and_then(|rules| rules.get(domain))
                    .filter(|raw_rule| raw_adblock_rule_can_be_copied(raw_rule.as_ref()))
                {
                    // No `$` options. Keep the stored line, including its casing.
                    encoder.write_all(raw_rule.as_ref().as_bytes())?;
                    encoder.write_all(b"\n")?;
                    return Ok(());
                }
                // No stored rule, a neutral `$` option, or an option that
                // would change the block. The raw text is not copied.
                encoder.write_all(b"||")?;
                encoder.write_all(domain.as_bytes())?;
                encoder.write_all(b"^\n")?;
            }
        }
        Ok(())
    }

    fn write_list<R: AsRef<str>>(
        &self,
        list_name: &str,
        format: OutputFormat,
        domains: &[String],
        adblock_rules: &HashMap<String, R>,
        control: &GenerationControl,
    ) -> Result<ManifestFile> {
        control.cancellation.check()?;
        let total_domains = domains.len() as u64;
        let filename = format!("{}_{}.txt.gz", list_name, format.as_str());
        (control.report)(GenerationEvent::FileStarted(format));

        let file = create_new_file(&self.output_dir.join(&filename))?;
        let stored = BufWriter::with_capacity(4 * 1024 * 1024, HashingWriter::new(file));
        let mut encoder = CountingWriter::new(GzEncoder::new(stored, Compression::fast()));

        let header = self.generate_header(format, total_domains);
        encoder.write_all(header.as_bytes())?;

        let rules_ref = if format == OutputFormat::Adblock {
            Some(adblock_rules)
        } else {
            None
        };
        for chunk in domains.chunks(WORK_GRAIN) {
            control.cancellation.check()?;
            for domain in chunk {
                Self::write_domain(&mut encoder, format, domain, rules_ref)?;
            }
            (control.report)(GenerationEvent::DomainsWritten(format, chunk.len() as u64));
        }

        let (compressor, uncompressed_bytes) = encoder.finish();
        let hashed = compressor
            .finish()?
            .into_inner()
            .map_err(|unflushed| unflushed.into_error())?;
        let (file, sha256, gz_size) = hashed.finish();
        file.sync_all()?;

        control.cancellation.check()?;
        (control.report)(GenerationEvent::FileCompleted { format, gz_size });

        info!(
            "Generated {} ({} domains, {} bytes compressed)",
            filename, total_domains, gz_size
        );

        Ok(ManifestFile {
            name: filename,
            list: list_name.to_string(),
            format: format.as_str().to_string(),
            domain_count: total_domains,
            size_bytes: gz_size,
            uncompressed_bytes,
            sha256,
        })
    }

    /// Generate all output formats in parallel (optimized)
    pub fn generate_all<R: AsRef<str> + Sync>(
        &self,
        domains: &[String],
        adblock_rules: &HashMap<String, R>,
        control: &GenerationControl,
    ) -> Result<Vec<ManifestFile>> {
        OutputFormat::all()
            .par_iter()
            .map(|format| {
                self.write_list(COMBINED_LIST_NAME, *format, domains, adblock_rules, control)
            })
            .collect()
    }

    /// Generate output files for all categories in parallel
    pub fn generate_all_categories<R: AsRef<str> + Sync>(
        &self,
        category_domains: &HashMap<Option<String>, Vec<String>>,
        adblock_rules: &HashMap<String, R>,
        control: &GenerationControl,
    ) -> Result<Vec<ManifestFile>> {
        category_list_names(category_domains.keys())?;

        // Build task list: (category, format) pairs with domain reference
        let tasks: Vec<(&str, OutputFormat, &[String])> = category_domains
            .iter()
            .flat_map(|(cat, domains)| {
                OutputFormat::all().into_iter().map(move |fmt| {
                    (
                        cat.as_deref().unwrap_or(UNCATEGORIZED_LIST_NAME),
                        fmt,
                        domains.as_slice(),
                    )
                })
            })
            .collect();

        info!(
            "Generating {} category files ({} categories × 3 formats)",
            tasks.len(),
            category_domains.len()
        );

        // Generate all files in parallel using rayon
        tasks
            .par_iter()
            .map(|(list_name, format, domains)| {
                self.write_list(list_name, *format, domains, adblock_rules, control)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::fs::{self, File};
    use tempfile::TempDir;

    const NO_RULES: Option<&HashMap<String, String>> = None;

    #[test]
    fn test_write_domain_hosts() {
        let mut buf = Vec::new();
        OutputGenerator::write_domain(&mut buf, OutputFormat::Hosts, "example.com", NO_RULES)
            .unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "0.0.0.0 example.com\n");
    }

    #[test]
    fn test_write_domain_plain() {
        let mut buf = Vec::new();
        OutputGenerator::write_domain(&mut buf, OutputFormat::Plain, "example.com", NO_RULES)
            .unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "example.com\n");
    }

    #[test]
    fn test_write_domain_adblock_no_rule() {
        let mut buf = Vec::new();
        // No adblock rule stored - generates default format
        OutputGenerator::write_domain(&mut buf, OutputFormat::Adblock, "example.com", NO_RULES)
            .unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "||example.com^\n");
    }

    #[test]
    fn test_write_domain_adblock_dollar_all_is_not_copied() {
        let mut rules = HashMap::new();
        rules.insert("x.example".to_string(), "||x.example^$all".to_string());
        let rules = Some(&rules);

        // Pi-hole v6 ignores a rule that still has `$all`, so the domain is
        // not blocked. The adblock line is `||domain^`. Hosts and plain
        // already blocked the domain and stay as they are.
        let mut adblock = Vec::new();
        OutputGenerator::write_domain(&mut adblock, OutputFormat::Adblock, "x.example", rules)
            .unwrap();
        assert_eq!(String::from_utf8(adblock).unwrap(), "||x.example^\n");

        let mut hosts = Vec::new();
        OutputGenerator::write_domain(&mut hosts, OutputFormat::Hosts, "x.example", rules).unwrap();
        assert_eq!(String::from_utf8(hosts).unwrap(), "0.0.0.0 x.example\n");

        let mut plain = Vec::new();
        OutputGenerator::write_domain(&mut plain, OutputFormat::Plain, "x.example", rules).unwrap();
        assert_eq!(String::from_utf8(plain).unwrap(), "x.example\n");
    }

    #[test]
    fn test_write_domain_adblock_with_rule() {
        let mut buf = Vec::new();
        let mut rules = HashMap::new();
        rules.insert(
            "example.com".to_string(),
            "||example.com^$important".to_string(),
        );

        // `$important` only outranks exception rules in a browser. Pi-hole v6
        // drops the whole line when the option is kept, and the whitelist has
        // already removed allowed domains, so there is no exception left to
        // outrank. Keeping the option would hide the block.
        OutputGenerator::write_domain(&mut buf, OutputFormat::Adblock, "example.com", Some(&rules))
            .unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "||example.com^\n");

        // AdGuard's DNS filter stores some of these without a caret. Pi-hole
        // requires the final character to be `^`, so the missing caret is
        // restored from the normalized domain.
        let mut rules = HashMap::new();
        rules.insert(
            "deloton.com".to_string(),
            "||deloton.com$important".to_string(),
        );
        let mut buf = Vec::new();
        OutputGenerator::write_domain(&mut buf, OutputFormat::Adblock, "deloton.com", Some(&rules))
            .unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "||deloton.com^\n");
    }

    #[test]
    fn test_write_domain_adblock_strips_combined_neutral_modifiers() {
        let mut buf = Vec::new();
        let mut rules = HashMap::new();
        rules.insert(
            "tracker.com".to_string(),
            "||tracker.com^$all,important".to_string(),
        );

        OutputGenerator::write_domain(&mut buf, OutputFormat::Adblock, "tracker.com", Some(&rules))
            .unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "||tracker.com^\n");
    }

    fn written_adblock(raw: &str) -> String {
        let mut rules = HashMap::new();
        rules.insert("x.example".to_string(), raw.to_string());
        let mut buf = Vec::new();
        OutputGenerator::write_domain(&mut buf, OutputFormat::Adblock, "x.example", Some(&rules))
            .unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn adblock_modifier_policy_strips_neutral_options_and_drops_the_rest() {
        let neutral = [
            "||x.example^",
            "||x.example^$all",
            "||x.example^$ALL",
            "||x.example^$important",
            "||x.example^$all,important",
            "||x.example^$document",
            "||x.example^$doc",
            "||x.example^$popup",
            "||x.example^$third-party",
            "||x.example^$3p",
            "||x.example^$document,popup,3p",
        ];
        // These change what a plain `||domain^` means. The extractor still
        // admits the ones it admits today; the raw text is not copied, and
        // the domain line matches the hosts and plain block.
        let changing = [
            "||x.example^$badfilter",
            "||x.example^$denyallow=keep.example",
            "||x.example^$client=127.0.0.1",
            "||x.example^$ctag=kids",
            "||x.example^$dnstype=A",
            "||x.example^$dnsrewrite=127.0.0.1",
            "||x.example^$domain=other.example",
            "||x.example^$~third-party",
            "||x.example^$all,~document",
        ];
        for raw in neutral {
            assert!(raw_adblock_rule_is_dns_neutral(raw), "{raw}");
            assert_eq!(written_adblock(raw), "||x.example^\n", "{raw}");
        }
        for raw in changing {
            assert!(!raw_adblock_rule_is_dns_neutral(raw), "{raw}");
            assert_eq!(written_adblock(raw), "||x.example^\n", "{raw}");
            assert!(!written_adblock(raw).contains('$'), "{raw}");
        }
    }

    fn collect_events(
        events: &std::sync::Mutex<Vec<GenerationEvent>>,
    ) -> impl Fn(GenerationEvent) + Send + Sync + '_ {
        move |event| events.lock().unwrap().push(event)
    }

    fn numbered_domains(count: usize) -> Vec<String> {
        (0..count).map(|n| format!("d{n:07}.example")).collect()
    }

    #[test]
    fn test_generate_all_writes_every_format() {
        let temp_dir = TempDir::new().unwrap();
        let generator = OutputGenerator::new(temp_dir.path());
        let domains = vec![
            "ads.example.com".to_string(),
            "plain.example.com".to_string(),
        ];
        let mut adblock_rules = HashMap::new();
        adblock_rules.insert(
            "ads.example.com".to_string(),
            "||ads.example.com^$important".to_string(),
        );
        let cancellation = Cancellation::default();
        let events = std::sync::Mutex::new(Vec::new());
        let report = collect_events(&events);

        let files = generator
            .generate_all(
                &domains,
                &adblock_rules,
                &GenerationControl {
                    cancellation: &cancellation,
                    report: &report,
                },
            )
            .unwrap();

        assert_eq!(
            files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            [
                "all_domains_hosts.txt.gz",
                "all_domains_plain.txt.gz",
                "all_domains_adblock.txt.gz"
            ]
        );
        for file in &files {
            let stored = fs::read(temp_dir.path().join(&file.name)).unwrap();
            let mut text = String::new();
            std::io::Read::read_to_string(
                &mut flate2::read::GzDecoder::new(stored.as_slice()),
                &mut text,
            )
            .unwrap();
            assert_eq!(file.list, "all_domains");
            assert_eq!(file.domain_count, 2);
            assert_eq!(file.size_bytes, stored.len() as u64);
            assert_eq!(file.uncompressed_bytes, text.len() as u64);
            assert_eq!(file.sha256, format!("{:x}", Sha256::digest(&stored)));
        }
        let mut adblock = String::new();
        std::io::Read::read_to_string(
            &mut flate2::read::GzDecoder::new(
                File::open(temp_dir.path().join("all_domains_adblock.txt.gz")).unwrap(),
            ),
            &mut adblock,
        )
        .unwrap();
        assert!(adblock.contains("||ads.example.com^\n||plain.example.com^\n"));
        assert!(!adblock.contains('$'));
    }

    #[test]
    fn every_category_file_reports_its_progress() {
        let temp_dir = TempDir::new().unwrap();
        let generator = OutputGenerator::new(temp_dir.path());
        let mut categories = HashMap::new();
        categories.insert(Some("tracking".to_string()), numbered_domains(20_000));
        categories.insert(None, numbered_domains(3));
        let cancellation = Cancellation::default();
        let events = std::sync::Mutex::new(Vec::new());
        let report = collect_events(&events);

        let files = generator
            .generate_all_categories(
                &categories,
                &HashMap::<String, String>::new(),
                &GenerationControl {
                    cancellation: &cancellation,
                    report: &report,
                },
            )
            .unwrap();

        assert_eq!(files.len(), 6);
        assert!(temp_dir.path().join("uncategorized_plain.txt.gz").exists());
        let events = events.lock().unwrap().clone();
        for format in OutputFormat::all() {
            let started = events
                .iter()
                .filter(|event| **event == GenerationEvent::FileStarted(format))
                .count();
            let written: u64 = events
                .iter()
                .filter_map(|event| match event {
                    GenerationEvent::DomainsWritten(reported, count) if *reported == format => {
                        Some(*count)
                    }
                    _ => None,
                })
                .sum();
            let compressed: u64 = events
                .iter()
                .filter_map(|event| match event {
                    GenerationEvent::FileCompleted {
                        format: reported,
                        gz_size,
                    } if *reported == format => Some(*gz_size),
                    _ => None,
                })
                .sum();
            assert_eq!(started, 2);
            assert_eq!(written, 20_003);
            assert_eq!(
                compressed,
                files
                    .iter()
                    .filter(|file| file.format == format.as_str())
                    .map(|file| file.size_bytes)
                    .sum::<u64>()
            );
        }
        assert!(
            events
                .iter()
                .filter(|event| matches!(event, GenerationEvent::DomainsWritten(..)))
                .count()
                > 6
        );
    }

    #[test]
    fn existing_file_is_never_overwritten() {
        let temp_dir = TempDir::new().unwrap();
        let generator = OutputGenerator::new(temp_dir.path());
        let existing = temp_dir.path().join("all_domains_hosts.txt.gz");
        fs::write(&existing, b"already written").unwrap();
        let cancellation = Cancellation::default();

        let outcome = generator.generate_all(
            &numbered_domains(3),
            &HashMap::<String, String>::new(),
            &GenerationControl {
                cancellation: &cancellation,
                report: &|_| {},
            },
        );

        assert!(outcome.is_err());
        assert_eq!(fs::read(&existing).unwrap(), b"already written");
    }

    #[test]
    fn category_names_that_collide_or_escape_the_directory_are_refused() {
        let safe = [Some("tracking".to_string()), None];
        assert_eq!(
            category_list_names(safe.iter()).unwrap(),
            ["tracking", "uncategorized"]
        );
        for unusable in [
            vec![Some("all_domains".to_string())],
            vec![Some("../escape".to_string())],
            vec![Some(String::new())],
            vec![Some("uncategorized".to_string()), None],
        ] {
            assert!(category_list_names(unusable.iter()).is_err());
        }

        let temp_dir = TempDir::new().unwrap();
        let mut categories = HashMap::new();
        categories.insert(Some("all_domains".to_string()), numbered_domains(1));
        let cancellation = Cancellation::default();
        let refused = OutputGenerator::new(temp_dir.path()).generate_all_categories(
            &categories,
            &HashMap::<String, String>::new(),
            &GenerationControl {
                cancellation: &cancellation,
                report: &|_| {},
            },
        );
        assert!(refused.is_err());
        assert_eq!(fs::read_dir(temp_dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn header_uses_the_instance_host() {
        use flate2::read::GzDecoder;
        use std::io::Read;

        fn file_text(path: &std::path::Path) -> String {
            let file = File::open(path).unwrap();
            let mut decoder = GzDecoder::new(file);
            let mut text = String::new();
            decoder.read_to_string(&mut text).unwrap();
            text
        }

        let domains = vec!["ads.example.com".to_string()];
        let rules = HashMap::<String, String>::new();
        let cancellation = Cancellation::default();
        let control = GenerationControl {
            cancellation: &cancellation,
            report: &|_| {},
        };

        let custom_dir = TempDir::new().unwrap();
        let custom = OutputGenerator::with_host(custom_dir.path(), "lists.example.test");
        let files = custom.generate_all(&domains, &rules, &control).unwrap();
        let hosts = files.iter().find(|file| file.format == "hosts").unwrap();
        let text = file_text(&custom_dir.path().join(&hosts.name));
        assert_eq!(
            text.lines().next().unwrap(),
            "# Blocklist generated by lists.example.test"
        );
        assert!(!text.contains(crate::generator_host::BLOCKLIST_GENERATOR_HOST));

        let fallback_dir = TempDir::new().unwrap();
        let fallback = OutputGenerator::new(fallback_dir.path());
        let fallback_files = fallback.generate_all(&domains, &rules, &control).unwrap();
        let plain = fallback_files
            .iter()
            .find(|file| file.format == "plain")
            .unwrap();
        let fallback_text = file_text(&fallback_dir.path().join(&plain.name));
        let fallback_host = crate::generator_host::BLOCKLIST_GENERATOR_HOST;
        assert_eq!(
            fallback_text.lines().next().unwrap(),
            format!("# Blocklist generated by {fallback_host}")
        );
    }
}
