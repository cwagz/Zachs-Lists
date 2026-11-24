"""Validate allowlist subscriptions. Rendering and the build digest stay in Rust."""

from collections.abc import Mapping, Sequence
from typing import Any

from app.allowlist_catalog import CatalogSource
from app.utils.validators import MAX_CONFIG_SOURCES


def allowlist_view(config: Mapping[str, Any] | None) -> dict[str, Any]:
    stored = config if isinstance(config, Mapping) else {}
    return {
        "apply_whitelist_to_blocklists": apply_whitelist_enabled(
            stored.get("apply_whitelist_to_blocklists")
        ),
        "allowlist_sources": normalize_allowlist_sources(
            stored.get("allowlist_sources")
        ),
        "allowlist_reports": normalize_allowlist_reports(
            stored.get("allowlist_reports")
        ),
    }


def apply_whitelist_enabled(value: object) -> bool:
    """Missing or non-boolean values keep today's filtering behavior."""
    return value if isinstance(value, bool) else True


def whitelist_applies_nowhere(apply_whitelist: bool, publish_allowlist: bool) -> bool:
    return not apply_whitelist and not publish_allowlist


def normalize_allowlist_sources(raw: object) -> list[dict[str, str]]:
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


def normalize_allowlist_reports(raw: object) -> list[dict[str, Any]]:
    if not isinstance(raw, list):
        return []
    reports: list[dict[str, Any]] = []
    for item in raw:
        if not isinstance(item, Mapping):
            continue
        url = _flat(item.get("url"))
        if not url:
            continue
        kept = item.get("kept")
        reports.append(
            {
                "url": url,
                "name": _flat(item.get("name")),
                "kept": (
                    kept
                    if isinstance(kept, int) and not isinstance(kept, bool)
                    else None
                ),
                "warning": _flat(item.get("warning")) or None,
                "used_cache": item.get("used_cache") is True,
            }
        )
    return reports


def resolve_allowlist_selection(
    requested: object,
    catalog: Sequence[CatalogSource | Mapping[str, str]],
    limit: int,
) -> tuple[list[dict[str, str]], str | None]:
    """Map requested URLs onto catalog entries, in request order."""
    if not isinstance(requested, list):
        return [], "allowlist sources must be a list of URLs"
    cap = min(limit, MAX_CONFIG_SOURCES)
    if len(requested) > cap:
        return [], f"Too many allowlist sources. Maximum: {cap}"

    by_url = {_catalog_url(entry): entry for entry in catalog}
    selected: list[dict[str, str]] = []
    seen: set[str] = set()
    for item in requested:
        if not isinstance(item, str):
            return [], "allowlist sources must be URLs"
        url = item.strip()
        if not url or url in seen:
            continue
        entry = by_url.get(url)
        if entry is None:
            return [], "Unknown allowlist source"
        seen.add(url)
        selected.append(_stored_source(entry))
    return selected, None


def _catalog_url(entry: CatalogSource | Mapping[str, str]) -> str:
    if isinstance(entry, CatalogSource):
        return entry.url
    return _flat(entry.get("url"))


def _stored_source(entry: CatalogSource | Mapping[str, str]) -> dict[str, str]:
    if isinstance(entry, CatalogSource):
        return {
            "url": entry.url,
            "name": entry.name,
            "license": entry.license,
            "project_url": entry.project_url,
        }
    return {
        "url": _flat(entry.get("url")),
        "name": _flat(entry.get("name")),
        "license": _flat(entry.get("license")),
        "project_url": _flat(entry.get("project_url")),
    }


def _flat(value: Any) -> str:
    if value is None:
        return ""
    return " ".join(str(value).replace("\t", " ").split())
