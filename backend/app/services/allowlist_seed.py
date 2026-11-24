"""Insert catalog allowlists when their URL is not already in the library."""

from datetime import UTC, datetime
from typing import Any

from pymongo.errors import DuplicateKeyError

from app.allowlist_catalog import LIST_TYPE_ALLOW, CatalogSource, load_catalog
from app.models.blocklist_library import BlocklistLibrary


def seed_document(entry: CatalogSource, now: datetime) -> dict[str, Any]:
    return {
        "url": entry.url,
        "name": entry.name,
        "category": "",
        "description": entry.description,
        "recommended": False,
        "aggressiveness": BlocklistLibrary.MIN_AGGRESSIVENESS,
        "domain_count": 0,
        "added_by": "catalog",
        "list_type": LIST_TYPE_ALLOW,
        "license": entry.license,
        "project_url": entry.project_url,
        "seed_key": entry.key,
        "created_at": now,
        "updated_at": now,
    }


def ensure_allowlist_catalog() -> None:
    """Insert each seeded URL once. An existing row is left as an admin saved it."""
    now = datetime.now(UTC)
    collection = BlocklistLibrary.get_collection()
    for entry in load_catalog():
        if collection.find_one({"url": entry.url}, {"_id": 1}):
            continue
        try:
            collection.insert_one(seed_document(entry, now))
        except DuplicateKeyError:
            continue


def allowlist_entries() -> list[BlocklistLibrary]:
    ensure_allowlist_catalog()
    return [
        entry
        for entry in BlocklistLibrary.get_all()
        if entry.list_type == LIST_TYPE_ALLOW
    ]
