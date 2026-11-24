//! Published allowlists are written by the API, not the blocklist pipeline.
//!
//! The list name matches `ALLOWLIST_NAME` in `backend/app/allowlist.py`.
//! These files live beside the generation directories. Legacy cleanup must
//! leave them in place: publication is opt-in and is not part of a build.

pub const ALLOWLIST_NAME: &str = "allowlist";

pub fn is_allowlist_artifact(filename: &str) -> bool {
    let name = file_name(filename);
    name.starts_with(&format!("{ALLOWLIST_NAME}_"))
        && (name.ends_with(".txt.gz") || name.ends_with(".txt"))
}

fn file_name(filename: &str) -> &str {
    filename.rsplit(['/', '\\']).next().unwrap_or(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_artifacts_are_not_blocklist_outputs() {
        assert!(is_allowlist_artifact("allowlist_adblock.txt.gz"));
        assert!(is_allowlist_artifact("allowlist_plain.txt.gz"));
        assert!(is_allowlist_artifact(
            "/data/users/sam/output/allowlist_adblock.txt.gz"
        ));
        assert!(!is_allowlist_artifact("all_domains_hosts.txt.gz"));
        assert!(!is_allowlist_artifact("all_domains_adblock.txt.gz"));
        assert!(!is_allowlist_artifact("advertising_hosts.txt.gz"));
        assert!(!is_allowlist_artifact("allowlist_adblock.txt.tmp"));
    }
}
