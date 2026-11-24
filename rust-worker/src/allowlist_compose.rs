//! Turn a downloaded allowlist into patterns and a published file.
//!
//! Fetch failures keep the source. A cached body is used and reported; with no
//! cache the source stays in the header with a warning instead of disappearing.

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::allowlist_render::{
    kept_rules, merged_whitelist, remove_published_allowlist, write_published_allowlist,
    AllowSourceBody,
};
use crate::downloader::DownloadResult;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AllowlistIdentity {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub project_url: String,
}

pub fn interpret_download(
    identity: &AllowlistIdentity,
    download: &DownloadResult,
) -> AllowSourceBody {
    if let Some(content) = &download.content {
        let warning = if download.stale_fallback {
            Some(download.warnings.first().cloned().unwrap_or_else(|| {
                format!(
                    "Allowlist fetch failed for {}; using cached copy",
                    display_name(identity)
                )
            }))
        } else {
            None
        };
        return AllowSourceBody {
            name: identity.name.clone(),
            url: identity.url.clone(),
            license: identity.license.clone(),
            project_url: identity.project_url.clone(),
            text: String::from_utf8_lossy(content).into_owned(),
            warning,
            content_hash: download.content_hash.clone(),
            used_cache: download.cache_hit || download.stale_fallback,
        };
    }
    AllowSourceBody {
        name: identity.name.clone(),
        url: identity.url.clone(),
        license: identity.license.clone(),
        project_url: identity.project_url.clone(),
        text: String::new(),
        warning: Some(format!(
            "Allowlist source unavailable: {} ({})",
            display_name(identity),
            download.error.as_deref().unwrap_or("download failed")
        )),
        content_hash: None,
        used_cache: false,
    }
}

pub fn filtering_whitelist(whitelist: &str, sources: &[AllowSourceBody], apply: bool) -> String {
    if apply {
        merged_whitelist(whitelist, sources)
    } else {
        String::new()
    }
}

pub fn publish_allowlist_files(
    output_dir: &std::path::Path,
    whitelist: &str,
    sources: &[AllowSourceBody],
    publish: bool,
    generated_at: DateTime<Utc>,
    generator: &str,
) -> anyhow::Result<()> {
    if publish {
        write_published_allowlist(output_dir, whitelist, sources, generated_at, generator)
    } else {
        remove_published_allowlist(output_dir)
    }
}

pub fn report_documents(sources: &[AllowSourceBody]) -> Vec<bson::Document> {
    sources
        .iter()
        .map(|source| {
            let mut document = bson::doc! {
                "url": &source.url,
                "name": &source.name,
                "kept": kept_rules(&source.text) as i64,
                "used_cache": source.used_cache,
            };
            if let Some(warning) = &source.warning {
                document.insert("warning", warning);
            }
            document
        })
        .collect()
}

fn display_name(identity: &AllowlistIdentity) -> String {
    if identity.name.trim().is_empty() {
        identity.url.clone()
    } else {
        identity.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allowlist_render::render_allowlist;
    use crate::cancellation::Cancellation;
    use crate::downloader::{DownloadResult, Source};
    use crate::extractor::DomainExtractor;
    use crate::generator::{GenerationControl, OutputGenerator};
    use crate::stages::{apply_whitelist, plan_whitelist, CategoryDomains};
    use flate2::read::GzDecoder;
    use std::collections::HashSet;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;

    const HOST: &str = "lists.example.test";
    const BLOCKLIST: &str = "\
ads.example.com
keep.example
track.example.net
referral.example.com
sub.allowed.example
";
    const SEEDED_WHITELIST: &str = "\
# seeded
ads.example.com
@@allowed.example
";

    fn moment() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-18T04:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn catalog_source(key: &str) -> AllowlistIdentity {
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../../backend/app/allowlist_catalog.json")).unwrap();
        let entry = document["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["key"] == key)
            .unwrap();
        AllowlistIdentity {
            url: entry["url"].as_str().unwrap().to_string(),
            name: entry["name"].as_str().unwrap().to_string(),
            license: entry["license"].as_str().unwrap().to_string(),
            project_url: entry["project_url"].as_str().unwrap().to_string(),
        }
    }

    fn referral_body() -> AllowSourceBody {
        let identity = catalog_source("hagezi-referral-native");
        AllowSourceBody {
            name: identity.name,
            url: identity.url,
            license: identity.license,
            project_url: identity.project_url,
            text: "\
! Title: synthetic referral allowlist
# comment
referral.example.com
@@||track.example.net^
||blocked.example^
not a domain
"
            .to_string(),
            warning: None,
            content_hash: Some("fresh".to_string()),
            used_cache: false,
        }
    }

    fn domains_of(text: &str) -> CategoryDomains {
        let extracted = DomainExtractor::new()
            .extract(text, false, &Cancellation::default(), &|_| {})
            .unwrap();
        let mut domains = HashSet::new();
        for batch in extracted.batches {
            for result in batch {
                domains.insert(result.domain);
            }
        }
        CategoryDomains {
            by_category: [(Some("advertising".to_string()), domains)]
                .into_iter()
                .collect(),
            adblock_rules: Default::default(),
        }
    }

    fn remaining(whitelist: &str, apply: bool, sources: &[AllowSourceBody]) -> HashSet<String> {
        let plan = plan_whitelist(
            &filtering_whitelist(whitelist, sources, apply),
            domains_of(BLOCKLIST),
            &Cancellation::default(),
        )
        .unwrap();
        let outcome = apply_whitelist(plan, &Cancellation::default(), &|_| {}).unwrap();
        outcome
            .domains
            .by_category
            .into_values()
            .flatten()
            .collect()
    }

    fn read_gzip(path: &Path) -> String {
        let mut text = String::new();
        GzDecoder::new(File::open(path).unwrap())
            .read_to_string(&mut text)
            .unwrap();
        text
    }

    fn rules(text: &str) -> Vec<&str> {
        text.lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    }

    fn failed_download(url: &str) -> DownloadResult {
        DownloadResult {
            source: Source {
                name: "cached".into(),
                url: url.into(),
                category: None,
                abp: false,
            },
            url_hash: "hash".into(),
            content: None,
            content_hash: None,
            last_checked_at: None,
            stale_fallback: false,
            cache_hit: false,
            bytes_downloaded: 0,
            download_time_ms: 1,
            error: Some("connection refused".into()),
            warnings: Vec::new(),
            previous_domain_count: None,
        }
    }

    #[test]
    fn synthetic_build_merges_subscriptions_and_respects_the_toggle() {
        let directory = tempfile::TempDir::new().unwrap();
        let sources = vec![referral_body()];
        let kept_on = remaining(SEEDED_WHITELIST, true, &sources);
        let kept_off = remaining(SEEDED_WHITELIST, false, &sources);
        assert_eq!(kept_on, HashSet::from(["keep.example".to_string()]));
        assert_eq!(kept_off.len(), 5);

        let mut plain_domains: Vec<String> = kept_on.into_iter().collect();
        plain_domains.sort();
        let adblock_rules: std::collections::HashMap<String, String> = Default::default();
        let cancellation = Cancellation::default();
        OutputGenerator::with_host(directory.path(), HOST)
            .generate_all(
                &plain_domains,
                &adblock_rules,
                &GenerationControl {
                    cancellation: &cancellation,
                    report: &|_| {},
                },
            )
            .unwrap();
        let blocklist = read_gzip(&directory.path().join("all_domains_plain.txt.gz"));
        assert!(blocklist.contains("keep.example"));
        assert!(!blocklist.contains("ads.example.com"));
        assert!(!blocklist.contains("referral.example.com"));
        assert!(blocklist.contains(&format!("Blocklist generated by {HOST}")));

        publish_allowlist_files(
            directory.path(),
            SEEDED_WHITELIST,
            &sources,
            true,
            moment(),
            HOST,
        )
        .unwrap();
        let allowlist = read_gzip(&directory.path().join("allowlist_adblock.txt.gz"));
        assert!(allowlist.contains(&format!("Allowlist generated by {HOST}")));
        assert!(allowlist.contains(
            "# Source: HaGeZi Allowlist Referral | GPL-3.0 | 2 entries | https://raw.githubusercontent.com/hagezi/dns-blocklists/main/adblock/whitelist-referral-native.txt"
        ));
        assert!(allowlist.contains("# Maintainer: https://github.com/hagezi/dns-blocklists"));
        assert!(allowlist.contains("# User whitelist: 2 entries"));
        assert!(allowlist.contains("# Dropped unsupported lines: 2"));
        assert_eq!(
            rules(&allowlist),
            vec![
                "ads.example.com",
                "@@||allowed.example^",
                "referral.example.com",
                "@@||track.example.net^",
            ]
        );

        let plain = render_allowlist(SEEDED_WHITELIST, "plain", moment(), HOST, &sources);
        assert_eq!(
            rules(&plain),
            vec!["ads.example.com", "referral.example.com"]
        );
        assert!(!directory.path().join("allowlist_hosts.txt.gz").exists());

        publish_allowlist_files(
            directory.path(),
            SEEDED_WHITELIST,
            &sources,
            false,
            moment(),
            HOST,
        )
        .unwrap();
        let blocklist_off = {
            let mut domains: Vec<String> = kept_off.into_iter().collect();
            domains.sort();
            let off = tempfile::TempDir::new().unwrap();
            let adblock_rules: std::collections::HashMap<String, String> = Default::default();
            let cancellation = Cancellation::default();
            OutputGenerator::with_host(off.path(), HOST)
                .generate_all(
                    &domains,
                    &adblock_rules,
                    &GenerationControl {
                        cancellation: &cancellation,
                        report: &|_| {},
                    },
                )
                .unwrap();
            let text = read_gzip(&off.path().join("all_domains_plain.txt.gz"));
            assert!(text.contains("ads.example.com"));
            assert!(text.contains("referral.example.com"));
            assert!(text.contains("track.example.net"));
            text
        };
        assert!(blocklist_off.contains("sub.allowed.example"));
        assert!(!directory.path().join("allowlist_adblock.txt.gz").exists());
    }

    #[test]
    fn failed_fetch_uses_the_cached_copy_and_keeps_a_missing_source_visible() {
        let identity = catalog_source("goodnessjson-pihole-whitelist");
        let mut cached = failed_download(&identity.url);
        cached.content = Some(b"cached.example.com\n".to_vec());
        cached.content_hash = Some("cached-hash".into());
        cached.stale_fallback = true;
        cached.cache_hit = true;
        cached.error = None;
        cached.warnings = vec![
            "Stale source: upstream check failed (connection refused); using a verified cache body"
                .into(),
        ];
        let body = interpret_download(&identity, &cached);
        assert!(body.warning.as_deref().unwrap().contains("cache"));
        assert!(body.text.contains("cached.example.com"));
        assert_eq!(body.content_hash.as_deref(), Some("cached-hash"));

        let directory = tempfile::TempDir::new().unwrap();
        let sources = vec![body];
        let plan = plan_whitelist(
            &filtering_whitelist("", &sources, true),
            domains_of("cached.example.com\nkeep.example\n"),
            &Cancellation::default(),
        )
        .unwrap();
        let outcome = apply_whitelist(plan, &Cancellation::default(), &|_| {}).unwrap();
        let kept: HashSet<String> = outcome
            .domains
            .by_category
            .into_values()
            .flatten()
            .collect();
        assert_eq!(kept, HashSet::from(["keep.example".to_string()]));
        publish_allowlist_files(directory.path(), "", &sources, true, moment(), HOST).unwrap();
        let text = read_gzip(&directory.path().join("allowlist_adblock.txt.gz"));
        assert!(text.contains("# Source warning: GoodnessJSON Pi-hole Whitelist |"));
        assert!(text.contains("using a verified cache body"));
        assert!(rules(&text).contains(&"cached.example.com"));

        let missing = interpret_download(&identity, &failed_download(&identity.url));
        assert!(missing.text.is_empty());
        assert!(missing
            .warning
            .as_deref()
            .unwrap()
            .contains("Allowlist source unavailable"));
        publish_allowlist_files(
            directory.path(),
            "mine.example\n",
            &[missing],
            true,
            moment(),
            HOST,
        )
        .unwrap();
        let unavailable = read_gzip(&directory.path().join("allowlist_adblock.txt.gz"));
        assert!(
            unavailable.contains("# Source: GoodnessJSON Pi-hole Whitelist | MIT | 0 entries |")
        );
        assert!(unavailable.contains("# Source warning: GoodnessJSON Pi-hole Whitelist |"));
        assert!(unavailable.contains("connection refused"));
        assert!(rules(&unavailable).contains(&"mine.example"));
    }
}
