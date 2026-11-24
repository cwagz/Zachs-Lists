//! Pi-hole v6 allowlist files.
//!
//! This is the only renderer. Python still publishes a user's own patterns on
//! save when no allowlist is subscribed; a build rewrites the file here so the
//! header uses the same generator host as the blocklists and includes sources.

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
    Regex::new(r"^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$")
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
    pub warning: Option<String>,
    pub content_hash: Option<String>,
    pub used_cache: bool,
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

struct SourceSummary {
    name: String,
    url: String,
    license: String,
    project_url: String,
    kept: usize,
    warning: Option<String>,
}

pub fn render_allowlist(
    whitelist: &str,
    output_format: &str,
    generated_at: DateTime<Utc>,
    generator: &str,
    sources: &[AllowSourceBody],
) -> String {
    let plain = output_format == "plain";
    let entries = parse_whitelist(whitelist);
    let mut rules = Vec::new();
    let mut seen = HashSet::new();
    let mut wildcard_notes = Vec::new();
    let mut omitted_subdomains = Vec::new();
    let mut unsupported = Vec::new();
    let mut regexes = Vec::new();
    let mut user_kept = 0usize;

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
                if seen.insert(rule.clone()) {
                    rules.push(rule);
                    user_kept += 1;
                }
            }
        }
    }

    let mut dropped = 0usize;
    let mut summaries = Vec::new();
    for source in sources {
        if source.warning.is_some() && source.text.is_empty() {
            summaries.push(SourceSummary {
                name: source.name.clone(),
                url: source.url.clone(),
                license: source.license.clone(),
                project_url: source.project_url.clone(),
                kept: 0,
                warning: source.warning.clone(),
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
            if seen.insert(rule.clone()) {
                rules.push(rule);
                kept += 1;
            }
        }
        summaries.push(SourceSummary {
            name: source.name.clone(),
            url: source.url.clone(),
            license: source.license.clone(),
            project_url: source.project_url.clone(),
            kept,
            warning: source.warning.clone(),
        });
    }

    let header = render_header(&HeaderParts {
        plain,
        generated_at,
        rule_count: rules.len(),
        wildcard_notes: &wildcard_notes,
        omitted_subdomains: &omitted_subdomains,
        unsupported: &unsupported,
        regexes: &regexes,
        generator,
        sources: if sources.is_empty() {
            None
        } else {
            Some((&summaries, dropped, user_kept))
        },
    });
    if rules.is_empty() {
        format!("{header}\n")
    } else {
        format!("{header}\n{}\n", rules.join("\n"))
    }
}

pub fn kept_rules(text: &str) -> usize {
    filter_allowlist_source(text).0.len()
}

pub fn subscription_patterns(text: &str) -> Vec<String> {
    filter_allowlist_source(text)
        .0
        .into_iter()
        .map(|rule| {
            rule.strip_prefix("@@||")
                .and_then(|rest| rest.strip_suffix('^'))
                .map(|domain| format!("@@{domain}"))
                .unwrap_or(rule)
        })
        .collect()
}

pub fn merged_whitelist(whitelist: &str, sources: &[AllowSourceBody]) -> String {
    let mut extra = Vec::new();
    for source in sources {
        if source.text.is_empty() {
            continue;
        }
        extra.extend(subscription_patterns(&source.text));
    }
    if extra.is_empty() {
        whitelist.to_string()
    } else {
        format!("{whitelist}\n{}", extra.join("\n"))
    }
}

pub fn write_published_allowlist(
    output_dir: &Path,
    whitelist: &str,
    sources: &[AllowSourceBody],
    generated_at: DateTime<Utc>,
    generator: &str,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    for output_format in ["adblock", "plain"] {
        let text = render_allowlist(whitelist, output_format, generated_at, generator, sources);
        write_gzip_atomic(
            &output_dir.join(format!("{ALLOWLIST_NAME}_{output_format}.txt.gz")),
            &text,
        )?;
    }
    for path in paths_for(output_dir, "hosts") {
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
    !domain.is_empty()
        && domain.len() <= 253
        && domain != "localhost"
        && !domain.ends_with(".local")
        && DOMAIN_PATTERN.is_match(domain)
}

fn parse_whitelist(whitelist: &str) -> Vec<UserEntry> {
    whitelist
        .lines()
        .filter_map(|line| {
            let pattern = line.split('#').next().unwrap_or("").trim();
            if pattern.is_empty() {
                None
            } else {
                Some(classify(pattern))
            }
        })
        .collect()
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

struct HeaderParts<'a> {
    plain: bool,
    generated_at: DateTime<Utc>,
    rule_count: usize,
    wildcard_notes: &'a [String],
    omitted_subdomains: &'a [String],
    unsupported: &'a [String],
    regexes: &'a [String],
    generator: &'a str,
    sources: Option<(&'a [SourceSummary], usize, usize)>,
}

fn render_header(parts: &HeaderParts<'_>) -> String {
    let mut lines = vec![
        format!("# Allowlist generated by {}", parts.generator),
        format!(
            "# Generated: {}",
            parts.generated_at.format("%Y-%m-%dT%H:%M:%SZ")
        ),
        format!("# Allow rules: {}", parts.rule_count),
    ];
    if let Some((sources, dropped, user_kept)) = parts.sources {
        lines.push(format!("# Dropped unsupported lines: {dropped}"));
        lines.push(format!("# User whitelist: {user_kept} entries"));
        for source in sources {
            let license = {
                let value = header_field(&source.license);
                if value.is_empty() {
                    "Unspecified".to_string()
                } else {
                    value
                }
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
            if let Some(warning) = &source.warning {
                lines.push(format!(
                    "# Source warning: {} | {}",
                    header_field(&source.name),
                    header_field(warning)
                ));
            }
        }
    }
    if parts.plain {
        lines.push("# Plain format lists exact domains only.".to_string());
        lines.push(
            "# It does not honor subdomain semantics: @@ and *. patterns are omitted.".to_string(),
        );
        lines.push("# The URL without a format parameter is the Pi-hole v6 allowlist.".to_string());
        push_patterns(
            &mut lines,
            "# Omitted subdomain patterns:",
            parts.omitted_subdomains,
        );
    } else {
        lines
            .push("# Pi-hole v6 subscribed allowlist. Exact domains are listed as-is.".to_string());
        lines.push(
            "# @@example.com (apex and subdomains) is emitted as @@||example.com^.".to_string(),
        );
        if !unique(parts.wildcard_notes).is_empty() {
            lines.push(
                "# Subdomain-only patterns (*.example.com) use that same exception.".to_string(),
            );
            push_patterns(
                &mut lines,
                "# Pi-hole v6 also allows the apex for @@||example.com^:",
                parts.wildcard_notes,
            );
        }
    }
    push_patterns(
        &mut lines,
        "# These patterns cannot be expressed in a Pi-hole v6 subscribed allowlist and were not emitted:",
        parts.unsupported,
    );
    push_patterns(
        &mut lines,
        "# Regex is not supported in subscribed lists. Add these locally with pihole --allow-regex:",
        parts.regexes,
    );
    lines.join("\n")
}

fn push_patterns(lines: &mut Vec<String>, label: &str, patterns: &[String]) {
    let patterns = unique(patterns);
    if patterns.is_empty() {
        return;
    }
    lines.push(label.to_string());
    lines.extend(patterns.into_iter().map(|pattern| format!("#   {pattern}")));
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
    items
        .iter()
        .filter(|item| seen.insert((*item).clone()))
        .cloned()
        .collect()
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

fn paths_for(output_dir: &Path, output_format: &str) -> Vec<PathBuf> {
    let base = output_dir.join(format!("{ALLOWLIST_NAME}_{output_format}.txt"));
    let gzip = PathBuf::from(format!("{}.gz", base.display()));
    let temporary = PathBuf::from(format!("{}.tmp", gzip.display()));
    vec![base, gzip, temporary]
}
