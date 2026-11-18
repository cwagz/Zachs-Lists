"""Write or remove a published allowlist in the list output directory."""

import hashlib
import os
from pathlib import Path

from flask import current_app

from app.allowlist import (
    ALLOWLIST_NAME,
    AllowlistSourceText,
    sync_published_allowlist,
)
from app.generator_host import host_from_public_url
from app.models.cache import CacheMetadata
from app.models.system_config import SystemConfig
from app.models.user import User


def _generator_host() -> str:
    return host_from_public_url(current_app.config.get("FRONTEND_URL"))


def _cached_text(url: str) -> str | None:
    """Read a source body from the worker download cache. Never fetches."""
    url_hash = hashlib.sha256(url.encode("utf-8")).hexdigest()
    cached = CacheMetadata.get_by_url_hash(url_hash)
    if cached is None:
        return None
    content = cached.content
    if content is None:
        return None
    return content.decode("utf-8", errors="replace")


def load_source_texts(
    sources: list[dict[str, str]],
) -> tuple[list[AllowlistSourceText] | None, bool]:
    """Pair selections with cached bodies.

    The bool is False when any selected source has not been downloaded yet.
    """
    if not sources:
        return None, True
    loaded: list[AllowlistSourceText] = []
    complete = True
    for source in sources:
        text = _cached_text(source["url"])
        if text is None:
            complete = False
            loaded.append(
                AllowlistSourceText(
                    name=source.get("name", ""),
                    url=source["url"],
                    license=source.get("license", ""),
                    project_url=source.get("project_url", ""),
                    error="not yet fetched",
                )
            )
            continue
        loaded.append(
            AllowlistSourceText(
                name=source.get("name", ""),
                url=source["url"],
                license=source.get("license", ""),
                project_url=source.get("project_url", ""),
                text=text,
            )
        )
    return loaded, complete


def _sync(
    output_dir: str, whitelist: str, enabled: bool, sources: list[dict[str, str]]
) -> None:
    texts, complete = load_source_texts(sources)
    directory = Path(output_dir)
    existing = directory / f"{ALLOWLIST_NAME}_adblock.txt.gz"
    if sources and not complete and enabled and existing.exists():
        return
    sync_published_allowlist(
        directory,
        whitelist,
        enabled,
        generator=_generator_host(),
        sources=texts,
    )


def sync_user_allowlist(user: User) -> None:
    _sync(
        user.get_output_dir(),
        user.get_config("whitelist.txt") or "",
        user.publish_allowlist,
        user.allowlist_sources,
    )


def sync_default_allowlist() -> None:
    output_dir = os.path.join(current_app.config["DEFAULT_DIR"], "output")
    _sync(
        output_dir,
        SystemConfig.get_default_whitelist() or "",
        SystemConfig.is_allowlist_published(),
        SystemConfig.allowlist_sources(),
    )
