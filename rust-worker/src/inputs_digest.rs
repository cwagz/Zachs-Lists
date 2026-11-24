use sha2::{Digest, Sha256};

use crate::allowlist_compose::AllowlistIdentity;
use crate::downloader::Source;

#[derive(Debug, Clone)]
pub struct AllowlistDigest<'a> {
    pub identities: &'a [AllowlistIdentity],
    pub content_hashes: &'a [Option<String>],
    pub apply_whitelist: bool,
    pub publish_allowlist: bool,
}

pub const PROCESSOR_VERSION: u32 = 1;
const FAILED_SOURCE_MARKER: &str = "source-failed";
const NO_CATEGORY_MARKER: &str = "";

fn feed(hasher: &mut Sha256, field: &str) {
    hasher.update((field.len() as u64).to_be_bytes());
    hasher.update(field.as_bytes());
}

fn feed_count(hasher: &mut Sha256, count: usize) {
    hasher.update((count as u64).to_be_bytes());
}

#[cfg(test)]
pub fn inputs_digest(
    processor_version: u32,
    sources: &[Source],
    whitelist_patterns: &[String],
    content_hashes: &[Option<String>],
    generator_host: &str,
) -> String {
    inputs_digest_with_allowlist(
        processor_version,
        sources,
        whitelist_patterns,
        content_hashes,
        generator_host,
        &AllowlistDigest {
            identities: &[],
            content_hashes: &[],
            apply_whitelist: true,
            publish_allowlist: false,
        },
    )
}

/// Same digest as [`inputs_digest`], plus allowlist selection when it changes the build.
///
/// No subscriptions and filtering left on add nothing, so an existing generation
/// still matches.
pub fn inputs_digest_with_allowlist(
    processor_version: u32,
    sources: &[Source],
    whitelist_patterns: &[String],
    content_hashes: &[Option<String>],
    generator_host: &str,
    allowlist: &AllowlistDigest<'_>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(processor_version.to_be_bytes());
    feed_sources(&mut hasher, sources, content_hashes);
    feed_count(&mut hasher, whitelist_patterns.len());
    for pattern in whitelist_patterns {
        feed(&mut hasher, pattern);
    }
    feed(&mut hasher, generator_host);
    feed_allowlist(&mut hasher, allowlist);
    format!("{:x}", hasher.finalize())
}

fn feed_sources(hasher: &mut Sha256, sources: &[Source], content_hashes: &[Option<String>]) {
    feed_count(hasher, sources.len());
    for (index, source) in sources.iter().enumerate() {
        feed(hasher, &source.url);
        feed(hasher, &source.name);
        hasher.update([u8::from(source.category.is_some())]);
        feed(
            hasher,
            source.category.as_deref().unwrap_or(NO_CATEGORY_MARKER),
        );
        hasher.update([u8::from(source.abp)]);
        let content_hash = content_hashes.get(index).and_then(Option::as_deref);
        hasher.update([u8::from(content_hash.is_some())]);
        feed(hasher, content_hash.unwrap_or(FAILED_SOURCE_MARKER));
    }
}

fn feed_allowlist(hasher: &mut Sha256, allowlist: &AllowlistDigest<'_>) {
    if allowlist.identities.is_empty() && allowlist.apply_whitelist {
        return;
    }
    hasher.update([u8::from(allowlist.apply_whitelist)]);
    hasher.update([u8::from(allowlist.publish_allowlist)]);
    let mut rows: Vec<(String, &AllowlistIdentity, Option<&str>)> = allowlist
        .identities
        .iter()
        .enumerate()
        .map(|(index, identity)| {
            (
                identity.url.clone(),
                identity,
                allowlist
                    .content_hashes
                    .get(index)
                    .and_then(Option::as_deref),
            )
        })
        .collect();
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    feed_count(hasher, rows.len());
    for (url, identity, content_hash) in rows {
        feed(hasher, &url);
        feed(hasher, &identity.name);
        feed(hasher, &identity.license);
        feed(hasher, &identity.project_url);
        hasher.update([u8::from(content_hash.is_some())]);
        feed(hasher, content_hash.unwrap_or(FAILED_SOURCE_MARKER));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::whitelist::WhitelistManager;

    const HOST: &str = "lists.zachlagden.uk";

    fn digest(
        processor_version: u32,
        sources: &[Source],
        whitelist_patterns: &[String],
        content_hashes: &[Option<String>],
    ) -> String {
        inputs_digest(
            processor_version,
            sources,
            whitelist_patterns,
            content_hashes,
            HOST,
        )
    }

    fn source(url: &str, name: &str, category: Option<&str>) -> Source {
        Source {
            name: name.into(),
            url: url.into(),
            category: category.map(String::from),
            abp: false,
        }
    }

    fn sources() -> Vec<Source> {
        vec![
            source(
                "https://lists.example.com/Ads.txt",
                "ads",
                Some("advertising"),
            ),
            source("https://lists.example.com/trackers.txt", "trackers", None),
        ]
    }

    fn hashes() -> Vec<Option<String>> {
        vec![Some("aa".repeat(32)), Some("bb".repeat(32))]
    }

    fn patterns(whitelist: &str) -> Vec<String> {
        WhitelistManager::parse(whitelist)
            .whitelist
            .patterns_as_strings()
    }

    fn baseline() -> String {
        digest(
            PROCESSOR_VERSION,
            &sources(),
            &patterns("allowed.example\n*.cdn.example\n"),
            &hashes(),
        )
    }

    #[test]
    fn identical_inputs_give_the_same_digest_on_every_run() {
        assert_eq!(baseline(), baseline());
        assert_eq!(baseline().len(), 64);
        assert_eq!(
            baseline(),
            digest(
                PROCESSOR_VERSION,
                &sources(),
                &patterns("# a comment\n*.cdn.example\n\nallowed.example\n"),
                &hashes(),
            )
        );
    }

    #[test]
    fn source_order_changes_the_digest() {
        let mut reordered = sources();
        reordered.reverse();
        let mut reordered_hashes = hashes();
        reordered_hashes.reverse();
        assert_ne!(
            baseline(),
            digest(
                PROCESSOR_VERSION,
                &reordered,
                &patterns("allowed.example\n*.cdn.example\n"),
                &reordered_hashes,
            )
        );
    }

    #[test]
    fn url_case_changes_the_digest() {
        let mut lowercased = sources();
        lowercased[0].url = lowercased[0].url.to_lowercase();
        assert_ne!(
            baseline(),
            digest(
                PROCESSOR_VERSION,
                &lowercased,
                &patterns("allowed.example\n*.cdn.example\n"),
                &hashes(),
            )
        );
    }

    #[test]
    fn source_name_and_category_change_the_digest() {
        let mut renamed = sources();
        renamed[1].name = "tracking".into();
        let mut recategorised = sources();
        recategorised[1].category = Some("tracking".into());
        let mut uncategorised_as_empty = sources();
        uncategorised_as_empty[1].category = Some(String::new());
        let whitelist = patterns("allowed.example\n*.cdn.example\n");
        for changed in [renamed, recategorised, uncategorised_as_empty] {
            assert_ne!(
                baseline(),
                digest(PROCESSOR_VERSION, &changed, &whitelist, &hashes())
            );
        }
    }

    #[test]
    fn abp_flag_changes_the_digest() {
        let mut flagged = sources();
        flagged[1].abp = true;
        let whitelist = patterns("allowed.example\n*.cdn.example\n");
        let flagged_digest = digest(PROCESSOR_VERSION, &flagged, &whitelist, &hashes());
        assert_ne!(baseline(), flagged_digest);
        flagged[1].abp = false;
        assert_eq!(
            baseline(),
            digest(PROCESSOR_VERSION, &flagged, &whitelist, &hashes())
        );
    }

    #[test]
    fn whitelist_change_changes_the_digest() {
        assert_ne!(
            baseline(),
            digest(
                PROCESSOR_VERSION,
                &sources(),
                &patterns("allowed.example\n"),
                &hashes(),
            )
        );
    }

    #[test]
    fn source_content_change_or_failure_changes_the_digest() {
        let whitelist = patterns("allowed.example\n*.cdn.example\n");
        let mut changed = hashes();
        changed[1] = Some("cc".repeat(32));
        let mut failed = hashes();
        failed[1] = None;
        let mut failed_marker_as_hash = hashes();
        failed_marker_as_hash[1] = Some(FAILED_SOURCE_MARKER.into());
        let digests: Vec<String> = [changed, failed, failed_marker_as_hash]
            .iter()
            .map(|content| digest(PROCESSOR_VERSION, &sources(), &whitelist, content))
            .collect();
        assert!(digests.iter().all(|digest| *digest != baseline()));
        assert_ne!(digests[1], digests[2]);
    }

    #[test]
    fn processor_version_changes_the_digest() {
        assert_ne!(
            baseline(),
            digest(
                PROCESSOR_VERSION + 1,
                &sources(),
                &patterns("allowed.example\n*.cdn.example\n"),
                &hashes(),
            )
        );
    }

    #[test]
    fn field_boundaries_cannot_be_shifted_between_fields() {
        let whitelist: Vec<String> = Vec::new();
        let joined = vec![source("https://lists.example.com/ab", "c", None)];
        let split = vec![source("https://lists.example.com/a", "bc", None)];
        let content = vec![Some("aa".repeat(32))];
        assert_ne!(
            digest(PROCESSOR_VERSION, &joined, &whitelist, &content),
            digest(PROCESSOR_VERSION, &split, &whitelist, &content)
        );
    }

    #[test]
    fn generator_host_changes_the_digest() {
        let whitelist = patterns("allowed.example\n*.cdn.example\n");
        let same = inputs_digest(PROCESSOR_VERSION, &sources(), &whitelist, &hashes(), HOST);
        let other = inputs_digest(
            PROCESSOR_VERSION,
            &sources(),
            &whitelist,
            &hashes(),
            "lists.example.test",
        );
        assert_eq!(same, baseline());
        assert_ne!(same, other);
    }

    #[test]
    fn default_allowlist_settings_do_not_change_the_digest() {
        let whitelist = patterns("allowed.example\n*.cdn.example\n");
        let historical = digest(PROCESSOR_VERSION, &sources(), &whitelist, &hashes());
        let unchanged = inputs_digest_with_allowlist(
            PROCESSOR_VERSION,
            &sources(),
            &whitelist,
            &hashes(),
            HOST,
            &AllowlistDigest {
                identities: &[],
                content_hashes: &[],
                apply_whitelist: true,
                publish_allowlist: true,
            },
        );
        assert_eq!(historical, unchanged);
        let toggled = inputs_digest_with_allowlist(
            PROCESSOR_VERSION,
            &sources(),
            &whitelist,
            &hashes(),
            HOST,
            &AllowlistDigest {
                identities: &[],
                content_hashes: &[],
                apply_whitelist: false,
                publish_allowlist: true,
            },
        );
        assert_ne!(historical, toggled);
    }
}
