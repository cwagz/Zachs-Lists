"""Rewrite stored blocklist URLs to the current adblock sources.

Nothing runs this automatically. The default is a dry run.

From a checkout, with PYTHONPATH=backend and MONGO_URI set:

    python backend/scripts/update_source_urls.py
    python backend/scripts/update_source_urls.py --apply

Inside the zachs-lists container:

    docker compose exec -T zachs-lists env PYTHONPATH=/app/backend \\
        python /app/backend/scripts/update_source_urls.py
    docker compose exec -T zachs-lists env PYTHONPATH=/app/backend \\
        python /app/backend/scripts/update_source_urls.py --apply
"""

import argparse
import os
from dataclasses import dataclass
from datetime import UTC, datetime
from typing import Any

from app.source_urls import REMOVALS, REPLACEMENTS
from pymongo import MongoClient
from pymongo.database import Database

LIBRARY = "blocklist_library"
USERS = "users"
SYSTEM_CONFIG = "system_config"
DEFAULT_CONFIG_ID = "default_config"


@dataclass(frozen=True)
class Change:
    collection: str
    owner: str
    action: str
    before: str
    after: str


@dataclass(frozen=True)
class ConfigWrite:
    collection: str
    identifier: Any
    original: str
    updated: str


def rewrite_config(content: str) -> tuple[str, list[tuple[str, str, str]]]:
    """Return updated text and (action, old url, new url) rows.

    Comment and blank lines stay as they are. A removed line is dropped.
    The text after the URL, including ``|name|category``, stays as written.
    """
    pieces: list[str] = []
    edits: list[tuple[str, str, str]] = []
    for raw in content.splitlines(keepends=True):
        ending = ""
        line = raw
        if raw.endswith("\r\n"):
            line, ending = raw[:-2], "\r\n"
        elif raw.endswith("\n"):
            line, ending = raw[:-1], "\n"
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            pieces.append(raw)
            continue
        url_part, separator, rest = line.partition("|")
        url = url_part.strip()
        if url in REMOVALS:
            edits.append(("remove", url, ""))
            continue
        replacement = REPLACEMENTS.get(url)
        if replacement is None:
            pieces.append(raw)
            continue
        prefix_length = len(url_part) - len(url_part.lstrip())
        prefix = url_part[:prefix_length]
        suffix_space = url_part[prefix_length + len(url) :]
        pieces.append(f"{prefix}{replacement}{suffix_space}{separator}{rest}{ending}")
        edits.append(("rewrite", url, replacement))
    return "".join(pieces), edits


def _keeper(docs: list[dict[str, Any]], desired: str) -> dict[str, Any]:
    already = [doc for doc in docs if doc.get("url") == desired]
    pool = already or docs
    return min(pool, key=lambda doc: (str(doc.get("name") or ""), str(doc.get("_id"))))


def _library_changes(database: Database) -> list[tuple[Change, dict[str, Any]]]:
    grouped: dict[str, list[dict[str, Any]]] = {}
    planned: list[tuple[Change, dict[str, Any]]] = []
    for doc in database[LIBRARY].find({}):
        url = doc.get("url")
        if not isinstance(url, str):
            continue
        if url in REMOVALS:
            planned.append(
                (
                    Change(
                        LIBRARY,
                        str(doc.get("name") or doc.get("_id")),
                        "remove",
                        url,
                        "",
                    ),
                    doc,
                )
            )
            continue
        grouped.setdefault(REPLACEMENTS.get(url, url), []).append(doc)
    for desired, docs in grouped.items():
        keep = _keeper(docs, desired)
        for doc in docs:
            name = str(doc.get("name") or doc.get("_id"))
            current = str(doc.get("url") or "")
            if doc is keep:
                if current != desired:
                    planned.append(
                        (Change(LIBRARY, name, "rewrite", current, desired), doc)
                    )
                continue
            planned.append(
                (
                    Change(
                        LIBRARY, name, "remove", current, str(keep.get("name") or "")
                    ),
                    doc,
                )
            )
    return planned


def _config_job(
    collection: str,
    identifier: Any,
    owner: str,
    content: str,
) -> tuple[list[Change], ConfigWrite | None]:
    updated, edits = rewrite_config(content)
    changes = [
        Change(collection, owner, action, before, after)
        for action, before, after in edits
    ]
    if not changes:
        return [], None
    return changes, ConfigWrite(collection, identifier, content, updated)


def collect(
    database: Database,
) -> tuple[list[tuple[Change, dict[str, Any]]], list[tuple[list[Change], ConfigWrite]]]:
    library = _library_changes(database)
    configs: list[tuple[list[Change], ConfigWrite]] = []
    default = database[SYSTEM_CONFIG].find_one({"_id": DEFAULT_CONFIG_ID})
    if default and isinstance(default.get("blocklists"), str) and default["blocklists"]:
        changes, write = _config_job(
            SYSTEM_CONFIG,
            DEFAULT_CONFIG_ID,
            DEFAULT_CONFIG_ID,
            default["blocklists"],
        )
        if write is not None:
            configs.append((changes, write))
    cursor = database[USERS].find({}, {"username": 1, "config.blocklists": 1})
    for user in sorted(
        cursor, key=lambda row: str(row.get("username") or row.get("_id"))
    ):
        content = user.get("config", {}).get("blocklists", "")
        if not isinstance(content, str) or not content:
            continue
        changes, write = _config_job(USERS, user["_id"], _owner(user), content)
        if write is not None:
            configs.append((changes, write))
    return library, configs


def _owner(user: dict[str, Any]) -> str:
    username = user.get("username")
    if isinstance(username, str) and username:
        return username
    return str(user.get("_id"))


def render(changes: list[Change], applied: bool) -> str:
    if not changes:
        if applied:
            return "Applied. No changes.\n"
        return "Dry run. No changes.\n"
    lines = [
        (
            "Applied."
            if applied
            else "Dry run. No writes. Pass --apply to update the database."
        )
    ]
    for change in changes:
        lines.append(f"[{change.collection}] {change.owner}: {change.action}")
        lines.append(f"  {change.before}")
        if change.action == "rewrite":
            lines.append(f"  -> {change.after}")
        elif change.after:
            lines.append(f"  duplicate of {change.after}")
    rewrites = sum(change.action == "rewrite" for change in changes)
    removes = sum(change.action == "remove" for change in changes)
    lines.append(
        f"{rewrites} rewrite, {removes} remove, applied={str(applied).lower()}"
    )
    return "\n".join(lines) + "\n"


def run(database: Database, apply: bool = False) -> dict[str, Any]:
    library, configs = collect(database)
    changes = [change for change, _doc in library]
    for group, _write in configs:
        changes.extend(group)
    if apply and (library or configs):
        _apply(database, library, configs)
    return {
        "changes": changes,
        "applied": apply,
        "rewrite": sum(change.action == "rewrite" for change in changes),
        "remove": sum(change.action == "remove" for change in changes),
    }


def _apply(
    database: Database,
    library: list[tuple[Change, dict[str, Any]]],
    configs: list[tuple[list[Change], ConfigWrite]],
) -> None:
    now = datetime.now(UTC)
    # Drop rows first so a rewritten URL cannot collide with one still stored.
    for change, doc in library:
        if change.action != "remove":
            continue
        deleted = database[LIBRARY].delete_one(
            {"_id": doc["_id"], "url": doc.get("url")}
        )
        if deleted.deleted_count != 1:
            raise ValueError("Library row changed during update; stopped")
    for change, doc in library:
        if change.action != "rewrite":
            continue
        updated = database[LIBRARY].update_one(
            {"_id": doc["_id"], "url": doc.get("url")},
            {"$set": {"url": change.after, "domain_count": 0, "updated_at": now}},
        )
        if updated.matched_count != 1:
            raise ValueError("Library row changed during update; stopped")
    for _group, write in configs:
        if write.collection == USERS:
            result = database[USERS].update_one(
                {"_id": write.identifier, "config.blocklists": write.original},
                {
                    "$set": {"config.blocklists": write.updated},
                    "$inc": {"config.revision": 1},
                },
            )
        else:
            result = database[SYSTEM_CONFIG].update_one(
                {"_id": write.identifier, "blocklists": write.original},
                {"$set": {"blocklists": write.updated}},
            )
        if result.matched_count != 1:
            raise ValueError("Configuration changed during update; stopped")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument(
        "--dry-run",
        action="store_true",
        help="Print every change. This is the default.",
    )
    group.add_argument("--apply", action="store_true", help="Write the changes.")
    args = parser.parse_args(argv)
    client: MongoClient[dict[str, Any]]
    with MongoClient(os.environ["MONGO_URI"], serverSelectionTimeoutMS=5000) as client:
        report = run(client.get_default_database(), apply=args.apply)
    print(render(report["changes"], report["applied"]), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
