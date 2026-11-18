"""Insert the public allowlist catalog into the source library."""

from datetime import UTC, datetime
from typing import Any

from app.allowlist_catalog import (
    LIST_TYPE_ALLOW,
    SEED_ALLOWLISTS,
    AllowlistCatalogEntry,
)
from app.models.blocklist_library import BlocklistLibrary


def seed_document(entry: AllowlistCatalogEntry, now: datetime) -> dict[str, Any]:
    """Fields written for one catalog allowlist."""
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
    """Upsert seeded allowlists. Safe to call on every library read."""
    now = datetime.now(UTC)
    collection = BlocklistLibrary.get_collection()
    for entry in SEED_ALLOWLISTS:
        fields = seed_document(entry, now)
        existing = collection.find_one({"url": entry.url})
        if existing:
            created_at = existing.get("created_at", now)
            added_by = existing.get("added_by") or "catalog"
            collection.update_one(
                {"_id": existing["_id"]},
                {
                    "$set": {
                        **fields,
                        "created_at": created_at,
                        "added_by": added_by,
                        "recommended": existing.get("recommended", False),
                        "aggressiveness": existing.get(
                            "aggressiveness", BlocklistLibrary.MIN_AGGRESSIVENESS
                        ),
                        "domain_count": existing.get("domain_count", 0),
                    }
                },
            )
            continue
        collection.insert_one(fields)
