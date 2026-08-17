"""
Worker liveness model for MongoDB.
"""

from datetime import datetime, timedelta
from typing import List, Dict, Any

from app.extensions import mongo


class Worker:
    """Liveness record written by each Rust worker on every heartbeat."""

    COLLECTION = "workers"

    STALE_AFTER_SECONDS = 60

    @classmethod
    def alive(
        cls, stale_after_seconds: int = STALE_AFTER_SECONDS
    ) -> List[Dict[str, Any]]:
        """
        List workers that have sent a heartbeat recently.

        Args:
            stale_after_seconds: Seconds since last heartbeat before a worker
                is considered dead

        Returns:
            Worker liveness documents
        """
        cutoff = datetime.utcnow() - timedelta(seconds=stale_after_seconds)
        return list(
            mongo.db[cls.COLLECTION].find(
                {"last_seen_at": {"$gte": cutoff}},
                {"_id": 0, "worker_id": 1, "last_seen_at": 1, "current_job_id": 1},
            )
        )

    @classmethod
    def count_alive(cls, stale_after_seconds: int = STALE_AFTER_SECONDS) -> int:
        """
        Count workers that have sent a heartbeat recently.

        Args:
            stale_after_seconds: Seconds since last heartbeat before a worker
                is considered dead

        Returns:
            Number of live workers
        """
        cutoff = datetime.utcnow() - timedelta(seconds=stale_after_seconds)
        return mongo.db[cls.COLLECTION].count_documents(
            {"last_seen_at": {"$gte": cutoff}}
        )

    @classmethod
    def prune_stale(cls, stale_after_seconds: int = 3600) -> int:
        """
        Remove registrations left behind by workers that died abruptly.

        Args:
            stale_after_seconds: Seconds since last heartbeat before a
                registration is removed

        Returns:
            Number of registrations removed
        """
        cutoff = datetime.utcnow() - timedelta(seconds=stale_after_seconds)
        result = mongo.db[cls.COLLECTION].delete_many({"last_seen_at": {"$lt": cutoff}})
        return result.deleted_count
