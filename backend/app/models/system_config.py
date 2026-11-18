"""
System configuration model for default blocklist settings.

Stores default blocklist configuration and whitelist in MongoDB
instead of filesystem.
"""

from datetime import datetime
from typing import Optional

from app.extensions import mongo


class SystemConfig:
    """System configuration stored in MongoDB."""

    COLLECTION = "system_config"
    DOC_ID = "default_config"

    @classmethod
    def get_default_blocklists(cls) -> Optional[str]:
        """Get default blocklist configuration."""
        doc = mongo.db[cls.COLLECTION].find_one({"_id": cls.DOC_ID})
        return doc.get("blocklists") if doc else None

    @classmethod
    def get_default_whitelist(cls) -> Optional[str]:
        """Get default whitelist configuration."""
        doc = mongo.db[cls.COLLECTION].find_one({"_id": cls.DOC_ID})
        return doc.get("whitelist") if doc else None

    @classmethod
    def get_default_config(cls) -> dict:
        """Get both blocklists and whitelist configs."""
        doc = mongo.db[cls.COLLECTION].find_one({"_id": cls.DOC_ID})
        if doc:
            return {
                "blocklists": doc.get("blocklists", ""),
                "whitelist": doc.get("whitelist", ""),
                "publish_allowlist": doc.get("publish_allowlist") is True,
                "apply_whitelist_to_blocklists": _apply_whitelist(doc),
                "allowlist_sources": _allowlist_sources(doc),
                "updated_at": doc.get("updated_at"),
                "updated_by": doc.get("updated_by"),
            }
        return {
            "blocklists": "",
            "whitelist": "",
            "publish_allowlist": False,
            "apply_whitelist_to_blocklists": True,
            "allowlist_sources": [],
            "updated_at": None,
            "updated_by": None,
        }

    @classmethod
    def is_allowlist_published(cls) -> bool:
        """Whether the default whitelist is published as a Pi-hole allowlist."""
        doc = mongo.db[cls.COLLECTION].find_one(
            {"_id": cls.DOC_ID}, {"publish_allowlist": 1}
        )
        if not doc:
            return False
        return doc.get("publish_allowlist") is True

    @classmethod
    def apply_whitelist_to_blocklists(cls) -> bool:
        """Default-list whitelist filtering. Missing values stay on."""
        doc = mongo.db[cls.COLLECTION].find_one(
            {"_id": cls.DOC_ID}, {"apply_whitelist_to_blocklists": 1}
        )
        if not doc:
            return True
        return _apply_whitelist(doc)

    @classmethod
    def allowlist_sources(cls) -> list:
        """Allowlist sources selected for the default list. Default is none."""
        doc = mongo.db[cls.COLLECTION].find_one(
            {"_id": cls.DOC_ID}, {"allowlist_sources": 1}
        )
        if not doc:
            return []
        return _allowlist_sources(doc)

    @classmethod
    def set_apply_whitelist_to_blocklists(cls, enabled: bool, updated_by: str) -> None:
        """Set whether the default blocklists are whitelist-filtered."""
        mongo.db[cls.COLLECTION].update_one(
            {"_id": cls.DOC_ID},
            {
                "$set": {
                    "apply_whitelist_to_blocklists": enabled,
                    "updated_at": datetime.utcnow(),
                    "updated_by": updated_by,
                },
                "$setOnInsert": {"blocklists": "", "whitelist": ""},
            },
            upsert=True,
        )

    @classmethod
    def set_allowlist_sources(cls, sources: list, updated_by: str) -> None:
        """Store the default list's opted-in allowlist sources."""
        from app.allowlist_selection import normalize_allowlist_sources

        mongo.db[cls.COLLECTION].update_one(
            {"_id": cls.DOC_ID},
            {
                "$set": {
                    "allowlist_sources": normalize_allowlist_sources(sources),
                    "updated_at": datetime.utcnow(),
                    "updated_by": updated_by,
                },
                "$setOnInsert": {"blocklists": "", "whitelist": ""},
            },
            upsert=True,
        )

    @classmethod
    def set_publish_allowlist(cls, enabled: bool, updated_by: str) -> None:
        """Set the default allowlist publication opt-in."""
        mongo.db[cls.COLLECTION].update_one(
            {"_id": cls.DOC_ID},
            {
                "$set": {
                    "publish_allowlist": enabled,
                    "updated_at": datetime.utcnow(),
                    "updated_by": updated_by,
                },
                "$setOnInsert": {"blocklists": "", "whitelist": ""},
            },
            upsert=True,
        )

    @classmethod
    def update_default_blocklists(cls, content: str, updated_by: str) -> None:
        """Update default blocklist configuration."""
        mongo.db[cls.COLLECTION].update_one(
            {"_id": cls.DOC_ID},
            {
                "$set": {
                    "blocklists": content,
                    "updated_at": datetime.utcnow(),
                    "updated_by": updated_by,
                },
                "$setOnInsert": {"whitelist": ""},
            },
            upsert=True,
        )

    @classmethod
    def update_default_whitelist(cls, content: str, updated_by: str) -> None:
        """Update default whitelist configuration."""
        mongo.db[cls.COLLECTION].update_one(
            {"_id": cls.DOC_ID},
            {
                "$set": {
                    "whitelist": content,
                    "updated_at": datetime.utcnow(),
                    "updated_by": updated_by,
                },
                "$setOnInsert": {"blocklists": ""},
            },
            upsert=True,
        )

    @classmethod
    def update_default_config(
        cls, blocklists: Optional[str], whitelist: Optional[str], updated_by: str
    ) -> None:
        """Update both blocklists and whitelist configs."""
        update = {
            "$set": {
                "updated_at": datetime.utcnow(),
                "updated_by": updated_by,
            }
        }
        if blocklists is not None:
            update["$set"]["blocklists"] = blocklists
        if whitelist is not None:
            update["$set"]["whitelist"] = whitelist

        mongo.db[cls.COLLECTION].update_one(
            {"_id": cls.DOC_ID},
            update,
            upsert=True,
        )


def _apply_whitelist(doc: dict) -> bool:
    from app.allowlist_selection import apply_whitelist_enabled

    return apply_whitelist_enabled(doc.get("apply_whitelist_to_blocklists"))


def _allowlist_sources(doc: dict) -> list:
    from app.allowlist_selection import normalize_allowlist_sources

    return normalize_allowlist_sources(doc.get("allowlist_sources"))
