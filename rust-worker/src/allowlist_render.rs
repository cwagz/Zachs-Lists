//! Pi-hole v6 allowlist composition.
//!
//! Header lines and kept rules match `backend/app/allowlist.py`. Subscribed
//! source files contribute only exact domains and `@@||domain^`. The user's
//! own whitelist still converts `@@example.com` and `*.example.com`.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::Result;
use chrono::{DateTime, Utc};
use flate2::write::GzEncoder;
use flate2::Compression;
use regex::Regex;

use crate::allowlist_files::ALLOWLIST_NAME;

static DOMAIN_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z0-9][a-zA-Z0-9-]{0,61}[a-zA-Z0-9]$",
    )
    .expect("domain pattern")
});

static EXCEPTION_RULE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@@\|\|([^/\s$|^]+)\^$").expect("exception pattern"));

#[derive(Debug, Clone)]
pub struct AllowSourceBody {
    pub name: String,
    pub url: String,
    pub license: String,
    pub project_url: String,
    pub text: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
struct SourceSummary {
    name: String,
    url: String,
    license: String,
    project_url: String,
    kept: usize,
    error: Option<String>,
}

#[derive(Clone, Copy)]
enum Kind {
    Exact,
    Subdomain,
    Wildcard,
    Unsupported,
    Regex,
}

struct UserEntry {
    kind: Kind,
    original: String,
    domain: Option<String>,
}

pub fn render_allowlist(
    whitelist: &str,
    output_format: &str,
    generated_at: DateTime<Utc>,
    generator: &str,
    sources: Option<&[AllowSourceBody]>,
) -> String {
    let entries = parse_whitelist(whitelist);
    let mut rules = Vec::new();
    let mut seen = HashSet::new();
    let mut wildcard_notes = Vec::new();
    let mut omitted_subdomains = Vec::new();
    let mut unsupported = Vec::new();
    let mut regexes = Vec::new();
    let mut user_kept = 0usize;
    let plain = output_format == "plain";

    for entry in &entries {
        match entry.kind {
            Kind::Regex => regexes.push(entry.original.clone()),
            Kind::Unsupported => unsupported.push(entry.original.clone()),
            Kind::Exact | Kind::Subdomain | Kind::Wildcard => {
                if matches!(entry.kind, Kind::Wildcard) {
                    wildcard_notes.push(entry.original.clone());
                }
                if matches!(entry.kind, Kind::Subdomain | Kind::Wildcard) {
                    omitted_subdomains.push(entry.original.clone());
                }
                if plain && !matches!(entry.kind, Kind::Exact) {
                    continue;
                }
                let Some(rule) = rule_for(entry) else {
                    continue;
                };
                if !seen.insert(rule.clone()) {
                    continue;
                }
                rules.push(rule);
                user_kept += 1;
            }
        }
    }

    let mut summaries = None;
    let mut dropped = 0usize;
    if let Some(sources) = sources.filter(|items| !items.is_empty()) {
        let mut rows = Vec::new();
        for source in sources {
            if source.error.is_some() {
                rows.push(SourceSummary {
                    name: source.name.clone(),
                    url: source.url.clone(),
                    license: source.license.clone(),
                    project_url: source.project_url.clone(),
                    kept: 0,
                    error: source.error.clone(),
                });
                continue;
            }
            let (source_rules, source_dropped) = filter_allowlist_source(&source.text);
            dropped += source_dropped;
            let mut kept = 0usize;
            for rule in source_rules {
                if plain && rule.starts_with("@@") {
                    continue;
                }
                if !seen.insert(rule.clone()) {
                    continue;
                }
                rules.push(rule);
                kept += 1;
            }
            rows.push(SourceSummary {
                name: source.name.clone(),
                url: source.url.clone(),
                license: source.license.clone(),
                project_url: source.project_url.clone(),
                kept,
                error: None,
            });
        }
        summaries = Some(rows);
    }

    let header = header(
        plain,
        generated_at,
        rules.len(),
        &wildcard_notes,
        &omitted_subdomains,
        &unsupported,
        &regexes,
        generator,
        summaries.as_deref(),
        dropped,
        summaries.as_ref().map(|_| user_kept),
    );
    if rules.is_empty() {
        return format!("{header}\n");
    }
    format!("{header}\n{}\n", rules.join("\n"))
}

pub fn write_published_allowlist(
    output_dir: &Path,
    whitelist: &str,
    sources: &[AllowSourceBody],
    generated_at: DateTime<Utc>,
    generator: &str,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    let source_arg = if sources.is_empty() {
        None
    } else {
        Some(sources)
    };
    for output_format in ["adblock", "plain"] {
        let text = render_allowlist(
            whitelist,
            output_format,
            generated_at,
            generator,
            source_arg,
        );
        write_gzip_atomic(
            &output_dir.join(format!("{ALLOWLIST_NAME}_{output_format}.txt.gz")),
            &text,
        )?;
    }
    for path in hosts_artifacts(output_dir) {
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

pub fn remove_published_allowlist(output_dir: &Path) -> Result<()> {
    for output_format in ["adblock", "plain", "hosts"] {
        for path in paths_for(output_dir, output_format) {
            if path.exists() {
                fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}

pub fn filter_allowlist_source(text: &str) -> (Vec<String>, usize) {
    let mut rules = Vec::new();
    let mut seen = HashSet::new();
    let mut dropped = 0usize;
    for line in text.lines() {
        let Some(token) = source_line_token(line) else {
            continue;
        };
        let Some(rule) = compatible_allow_rule(&token) else {
            dropped += 1;
            continue;
        };
        if seen.insert(rule.clone()) {
            rules.push(rule);
        }
    }
    (rules, dropped)
}

fn source_line_token(line: &str) -> Option<String> {
    let mut stripped = line.trim();
    if stripped.is_empty() {
        return None;
    }
    let first = stripped.chars().next()?;
    if matches!(first, '#' | '!' | ';' | '[') {
        return None;
    }
    for marker in [" #", " !", "\t#", "\t!"] {
        if let Some(index) = stripped.find(marker) {
            stripped = stripped[..index].trim();
            break;
        }
    }
    if stripped.is_empty() {
        None
    } else {
        Some(stripped.to_string())
    }
}

fn compatible_allow_rule(token: &str) -> Option<String> {
    if token.chars().any(char::is_whitespace) {
        return None;
    }
    let lowered = token.to_lowercase();
    if let Some(captures) = EXCEPTION_RULE.captures(&lowered) {
        let domain = captures.get(1)?.as_str();
        if valid_domain(domain) {
            return Some(format!("@@||{domain}^"));
        }
        return None;
    }
    if lowered.contains('*') || lowered.starts_with("@@") || lowered.starts_with("||") {
        return None;
    }
    if valid_domain(&lowered) {
        return Some(lowered);
    }
    None
}

fn valid_domain(domain: &str) -> bool {
    if domain.is_empty()
        || domain.len() > 253
        || domain == "localhost"
        || domain.ends_with(".local")
    {
        return false;
    }
    DOMAIN_PATTERN.is_match(domain)
}

fn parse_whitelist(whitelist: &str) -> Vec<UserEntry> {
    let mut entries = Vec::new();
    for line in whitelist.lines() {
        let pattern = line.split('#').next().unwrap_or("").trim();
        if pattern.is_empty() {
            continue;
        }
        entries.push(classify(pattern));
    }
    entries
}

fn classify(pattern: &str) -> UserEntry {
    if pattern.len() > 2 && pattern.starts_with('/') && pattern.ends_with('/') {
        return UserEntry {
            kind: Kind::Regex,
            original: pattern.to_string(),
            domain: None,
        };
    }
    if let Some(rest) = pattern.strip_prefix("@@") {
        let domain = rest.to_lowercase();
        if valid_domain(&domain) {
            return UserEntry {
                kind: Kind::Subdomain,
                original: pattern.to_string(),
                domain: Some(domain),
            };
        }
        return UserEntry {
            kind: Kind::Unsupported,
            original: pattern.to_string(),
            domain: None,
        };
    }
    if pattern.contains('*') {
        if let Some(remainder) = pattern.strip_prefix("*.") {
            let domain = remainder.to_lowercase();
            if !remainder.contains('*') && valid_domain(&domain) {
                return UserEntry {
                    kind: Kind::Wildcard,
                    original: pattern.to_string(),
                    domain: Some(domain),
                };
            }
        }
        return UserEntry {
            kind: Kind::Unsupported,
            original: pattern.to_string(),
            domain: None,
        };
    }
    let domain = pattern.to_lowercase();
    if valid_domain(&domain) {
        return UserEntry {
            kind: Kind::Exact,
            original: pattern.to_string(),
            domain: Some(domain),
        };
    }
    UserEntry {
        kind: Kind::Unsupported,
        original: pattern.to_string(),
        domain: None,
    }
}

fn rule_for(entry: &UserEntry) -> Option<String> {
    let domain = entry.domain.as_deref()?;
    match entry.kind {
        Kind::Exact => Some(domain.to_string()),
        Kind::Subdomain | Kind::Wildcard => Some(format!("@@||{domain}^")),
        Kind::Unsupported | Kind::Regex => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn header(
    plain: bool,
    generated_at: DateTime<Utc>,
    rule_count: usize,
    wildcard_notes: &[String],
    omitted_subdomains: &[String],
    unsupported: &[String],
    regexes: &[String],
    generator: &str,
    sources: Option<&[SourceSummary]>,
    dropped: usize,
    user_kept: Option<usize>,
) -> String {
    let mut lines = vec![
        format!("# Allowlist generated by {generator}"),
        format!("# Generated: {}", generated_at.format("%Y-%m-%dT%H:%M:%SZ")),
        format!("# Allow rules: {rule_count}"),
    ];
    if let Some(sources) = sources {
        lines.push(format!("# Dropped unsupported lines: {dropped}"));
        if let Some(user_kept) = user_kept {
            lines.push(format!("# User whitelist: {user_kept} entries"));
        }
        for source in sources {
            let license = header_field(&source.license);
            let license = if license.is_empty() {
                "Unspecified".to_string()
            } else {
                license
            };
            lines.push(format!(
                "# Source: {} | {license} | {} entries | {}",
                header_field(&source.name),
                source.kept,
                header_field(&source.url)
            ));
            if !source.project_url.is_empty() {
                lines.push(format!(
                    "# Maintainer: {}",
                    header_field(&source.project_url)
                ));
            }
            if source.error.is_some() {
                lines.push(format!(
                    "# Source unavailable: {} | {}",
                    header_field(&source.name),
                    header_field(&source.url)
                ));
            }
        }
    }
    if plain {
        lines.push("# Plain format lists exact domains only.".to_string());
        lines.push(
            "# It does not honor subdomain semantics: @@ and *. patterns are omitted.".to_string(),
        );
        lines.push("# The URL without a format parameter is the Pi-hole v6 allowlist.".to_string());
        let omitted = unique(omitted_subdomains);
        if !omitted.is_empty() {
            lines.push("# Omitted subdomain patterns:".to_string());
            lines.extend(omitted.into_iter().map(|pattern| format!("#   {pattern}")));
        }
    } else {
        lines
            .push("# Pi-hole v6 subscribed allowlist. Exact domains are listed as-is.".to_string());
        lines.push(
            "# @@example.com (apex and subdomains) is emitted as @@||example.com^.".to_string(),
        );
        let notes = unique(wildcard_notes);
        if !notes.is_empty() {
            lines.push(
                "# Subdomain-only patterns (*.example.com) use that same exception.".to_string(),
            );
            lines.push("# Pi-hole v6 also allows the apex for @@||example.com^:".to_string());
            lines.extend(notes.into_iter().map(|pattern| format!("#   {pattern}")));
        }
    }
    let unsupported = unique(unsupported);
    if !unsupported.is_empty() {
        lines.push(
            "# These patterns cannot be expressed in a Pi-hole v6 subscribed allowlist and were not emitted:"
                .to_string(),
        );
        lines.extend(
            unsupported
                .into_iter()
                .map(|pattern| format!("#   {pattern}")),
        );
    }
    let regexes = unique(regexes);
    if !regexes.is_empty() {
        lines.push(
            "# Regex is not supported in subscribed lists. Add these locally with pihole --allow-regex:"
                .to_string(),
        );
        lines.extend(regexes.into_iter().map(|pattern| format!("#   {pattern}")));
    }
    format!("{}\n", lines.join("\n"))
}

fn header_field(value: &str) -> String {
    value
        .replace(['|', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn unique(items: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            unique.push(item.clone());
        }
    }
    unique
}

fn write_gzip_atomic(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = PathBuf::from(format!("{}.tmp", path.display()));
    let file = File::create(&temporary)?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(text.as_bytes())?;
    encoder.finish()?.sync_all()?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn hosts_artifacts(output_dir: &Path) -> Vec<PathBuf> {
    paths_for(output_dir, "hosts")
}

fn paths_for(output_dir: &Path, output_format: &str) -> Vec<PathBuf> {
    let base = output_dir.join(format!("{ALLOWLIST_NAME}_{output_format}.txt"));
    let gzip = PathBuf::from(format!("{}.gz", base.display()));
    let temporary = PathBuf::from(format!("{}.tmp", gzip.display()));
    vec![base, gzip, temporary]
}

#[cfg(test)]
fn read_gzip(path: &Path) -> String {
    use flate2::read::GzDecoder;
    use std::io::{BufReader, Read};

    let file = File::open(path).unwrap();
    let mut decoder = GzDecoder::new(BufReader::new(file));
    let mut text = String::new();
    decoder.read_to_string(&mut text).unwrap();
    text
}

#[cfg(test)]
fn rules(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn moment() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-18T04:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn referral() -> AllowSourceBody {
        AllowSourceBody {
            name: "HaGeZi Allowlist Referral".to_string(),
            url: "https://lists.example.test/referral.txt".to_string(),
            license: "GPL-3.0".to_string(),
            project_url: "https://github.com/hagezi/dns-blocklists".to_string(),
            text: [
                "! Title: synthetic referral allowlist",
                "# comment",
                "",
                "Example.COM",
                "example.com",
                "@@||Track.Example.net^",
                "@@||track.example.net^",
                "||blocked.example^",
                "0.0.0.0 hosts.example",
                "@@not-a-pihole-rule.example",
                "*.wildcard.example",
                "not a domain",
                "@@||bad_domain^.example^",
            ]
            .join("\n"),
            error: None,
        }
    }

    fn second() -> AllowSourceBody {
        AllowSourceBody {
            name: "GoodnessJSON Pi-hole Whitelist".to_string(),
            url: "https://lists.example.test/goodness.txt".to_string(),
            license: "MIT".to_string(),
            project_url: "https://github.com/GoodnessJSON/PiHole-Whitelist".to_string(),
            text: "example.com\nplain.example.org\n@@||track.example.net^\n".to_string(),
            error: None,
        }
    }

    #[test]
    fn source_filter_keeps_only_pihole_v6_rules() {
        let (rules, dropped) = filter_allowlist_source(&referral().text);
        assert_eq!(
            rules,
            vec![
                "example.com".to_string(),
                "@@||track.example.net^".to_string()
            ]
        );
        assert_eq!(dropped, 6);
    }

    #[test]
    fn merge_dedupes_and_header_records_sources() {
        let sources = vec![referral(), second()];
        let rendered = render_allowlist(
            "mine.example\n@@Ads.Example.com\nexample.com\n",
            "adblock",
            moment(),
            "lists.example.test",
            Some(&sources),
        );
        assert_eq!(
            rules(&rendered),
            vec![
                "mine.example",
                "@@||ads.example.com^",
                "example.com",
                "@@||track.example.net^",
                "plain.example.org",
            ]
        );

        let header_text = render_allowlist(
            "mine.example\n",
            "adblock",
            moment(),
            "lists.example.test",
            Some(&sources),
        );
        assert!(header_text.contains("Allowlist generated by lists.example.test"));
        assert!(header_text.contains("# Dropped unsupported lines: 6"));
        assert!(header_text.contains("# User whitelist: 1 entries"));
        assert!(header_text.contains(
            "# Source: HaGeZi Allowlist Referral | GPL-3.0 | 2 entries | https://lists.example.test/referral.txt"
        ));
        assert!(header_text.contains("# Maintainer: https://github.com/hagezi/dns-blocklists"));
        assert!(header_text.contains(
            "# Source: GoodnessJSON Pi-hole Whitelist | MIT | 1 entries | https://lists.example.test/goodness.txt"
        ));
    }

    #[test]
    fn plain_format_omits_exception_rules() {
        let sources = vec![referral()];
        let rendered = render_allowlist(
            "mine.example\n",
            "plain",
            moment(),
            "lists.example.test",
            Some(&sources),
        );
        assert_eq!(rules(&rendered), vec!["mine.example", "example.com"]);
        assert!(rendered.contains("# Source: HaGeZi Allowlist Referral | GPL-3.0 | 1 entries | "));
        assert!(!rules(&rendered).iter().any(|rule| rule.starts_with("@@||")));
    }

    #[test]
    fn write_uses_a_temp_directory_and_counts_a_failed_source() {
        let directory = TempDir::new().unwrap();
        let broken = AllowSourceBody {
            name: "Missing".to_string(),
            url: "https://lists.example.test/missing.txt".to_string(),
            license: "MIT".to_string(),
            project_url: "https://example.test/missing".to_string(),
            text: String::new(),
            error: Some("not yet fetched".to_string()),
        };
        write_published_allowlist(
            directory.path(),
            "mine.example\n",
            &[broken],
            moment(),
            "lists.example.test",
        )
        .unwrap();
        let text = read_gzip(&directory.path().join("allowlist_adblock.txt.gz"));
        assert!(text.contains(
            "# Source: Missing | MIT | 0 entries | https://lists.example.test/missing.txt"
        ));
        assert!(
            text.contains("# Source unavailable: Missing | https://lists.example.test/missing.txt")
        );
        assert!(text.contains("# Dropped unsupported lines: 0"));
        assert_eq!(rules(&text), vec!["mine.example"]);
        assert!(!directory.path().join("allowlist_hosts.txt.gz").exists());
        let plain = read_gzip(&directory.path().join("allowlist_plain.txt.gz"));
        assert_eq!(rules(&plain), vec!["mine.example"]);

        remove_published_allowlist(directory.path()).unwrap();
        assert!(directory.path().read_dir().unwrap().next().is_none());
    }
}
