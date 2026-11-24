"""Write or remove a published allowlist in the list output directory."""

import os

from flask import current_app

from app.allowlist import sync_published_allowlist
from app.generator_host import generator_host_from_env
from app.models.system_config import SystemConfig
from app.models.user import User


def _generator_host() -> str:
    """Same host the worker stamps. An explicit override wins over FRONTEND_URL."""
    return generator_host_from_env()


def sync_user_allowlist(user: User) -> None:
    # Subscribed sources are rendered by the worker. Rewriting here would
    # publish the user's lines without those sources.
    if user.publish_allowlist and _has_subscriptions(user.config):
        return
    sync_published_allowlist(
        user.get_output_dir(),
        user.get_config("whitelist.txt") or "",
        user.publish_allowlist,
        generator=_generator_host(),
    )


def sync_default_allowlist() -> None:
    config = SystemConfig.get_default_config()
    output_dir = os.path.join(current_app.config["DEFAULT_DIR"], "output")
    if config["publish_allowlist"] is True and _has_subscriptions(config):
        return
    sync_published_allowlist(
        output_dir,
        config["whitelist"] or "",
        config["publish_allowlist"] is True,
        generator=_generator_host(),
    )


def _has_subscriptions(config: object) -> bool:
    if not isinstance(config, dict):
        return False
    sources = config.get("allowlist_sources")
    return isinstance(sources, list) and len(sources) > 0
