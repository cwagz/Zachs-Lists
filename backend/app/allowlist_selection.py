"""Selected allowlist sources and the build identity they share with the worker.

The hash string must stay identical to ``config_hash_material`` in
``rust-worker/src/build_stamp.rs``. When no allowlist sources are selected and
whitelist filtering stays on, the string is the historical blocklist hash so
unchanged users still skip a manual rebuild.
"""

from collections.abc import Mapping, Sequence
from typing import Any

SOURCE_FIELDS = ("url", "name", "license", "project_url")


def apply_whitelist_enabled(value: object) -> bool:
    """Missing or non-boolean values keep today's filtering behavior."""
    if isinstance(value, bool):
        return value
    return True


def normalize_allowlist_sources(raw: object) -> list[dict[str, str]]:
    """Return stored selections as url/name/license/project_url dicts."""
    if not isinstance(raw, list):
        return []
    sources: list[dict[str, str]] = []
    seen: set[str] = set()
    for item in raw:
        if not isinstance(item, Mapping):
            continue
        url = _flat(item.get("url"))
        if not url or url in seen:
            continue
        seen.add(url)
        sources.append(
            {
                "url": url,
                "name": _flat(item.get("name")),
                "license": _flat(item.get("license")),
                "project_url": _flat(item.get("project_url")),
            }
        )
    return sources


def resolve_allowlist_selection(
    requested: object,
    catalog: Sequence[Mapping[str, str]],
    limit: int,
) -> tuple[list[dict[str, str]], str | None]:
    """Map requested URLs onto catalog entries, preserving request order."""
    if not isinstance(requested, list):
        return [], "allowlist sources must be a list of URLs"
    if len(requested) > limit:
        return [], f"Too many allowlist sources. Maximum: {limit}"

    by_url = {_flat(entry.get("url")): entry for entry in catalog}
    selected: list[dict[str, str]] = []
    seen: set[str] = set()
    for item in requested:
        if not isinstance(item, str):
            return [], "allowlist sources must be URLs"
        url = item.strip()
        if url in seen:
            continue
        entry = by_url.get(url)
        if entry is None:
            return [], "Unknown allowlist source"
        seen.add(url)
        selected.append(
            {
                "url": url,
                "name": _flat(entry.get("name")),
                "license": _flat(entry.get("license")),
                "project_url": _flat(entry.get("project_url")),
            }
        )
    return selected, None


def config_hash_material(
    blocklists: str,
    whitelist: str,
    sources: Sequence[Mapping[str, str]],
    apply_whitelist: bool,
    publish_allowlist: bool,
) -> str:
    """Identity of a build for the worker's no-changes skip."""
    base = f"{blocklists}\n---SEPARATOR---\n{whitelist}"
    canonical = _canonical_source_lines(sources)
    if not canonical and apply_whitelist:
        return base
    lines = [
        base,
        "---ALLOWLIST-SOURCES---",
        *canonical,
        "---APPLY-WHITELIST---",
        "1" if apply_whitelist else "0",
    ]
    if canonical:
        lines.extend(
            [
                "---PUBLISH-ALLOWLIST---",
                "1" if publish_allowlist else "0",
            ]
        )
    return "\n".join(lines)


def config_hash(
    blocklists: str,
    whitelist: str,
    sources: Sequence[Mapping[str, str]],
    apply_whitelist: bool,
    publish_allowlist: bool,
) -> str:
    import hashlib

    material = config_hash_material(
        blocklists, whitelist, sources, apply_whitelist, publish_allowlist
    )
    return hashlib.sha256(material.encode("utf-8")).hexdigest()


def _canonical_source_lines(sources: Sequence[Mapping[str, str]]) -> list[str]:
    rows: list[tuple[str, str]] = []
    seen: set[str] = set()
    for source in sources:
        url = _flat(source.get("url"))
        if not url or url in seen:
            continue
        seen.add(url)
        rows.append(
            (
                url,
                "\t".join(
                    [
                        url,
                        _flat(source.get("name")),
                        _flat(source.get("license")),
                        _flat(source.get("project_url")),
                    ]
                ),
            )
        )
    rows.sort(key=lambda item: item[0])
    return [line for _, line in rows]


def _flat(value: Any) -> str:
    if value is None:
        return ""
    return " ".join(str(value).replace("\t", " ").split())
