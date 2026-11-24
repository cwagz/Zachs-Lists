"""Public allowlist catalog shared with the worker tests.

The entries live in ``allowlist_catalog.json`` next to this module, so the
runtime image ships them with the backend package. Python seeds and stores
those fields. The worker reads the stored fields and is the only place that
renders an allowlist or folds the selection into the build digest.
"""

import json
import logging
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path

LIST_TYPE_BLOCK = "block"
LIST_TYPE_ALLOW = "allow"

NOWHERE_CODE = "whitelist_applies_nowhere"
NOWHERE_MESSAGE = (
    "Turn on “Apply whitelist to blocklists” or publish the allowlist. "
    "Otherwise the whitelist would not apply anywhere."
)

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class CatalogSource:
    key: str
    name: str
    url: str
    description: str
    license: str
    project_url: str


def catalog_path() -> Path:
    """Path shipped inside the backend package, not under tests/."""
    return Path(__file__).resolve().with_name("allowlist_catalog.json")


@lru_cache(maxsize=1)
def load_catalog() -> tuple[CatalogSource, ...]:
    """Return seeded sources. A missing file is an empty catalog, not an error."""
    path = catalog_path()
    try:
        raw = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        logger.error("Allowlist catalog file is missing: %s", path)
        return ()
    document = json.loads(raw)
    return tuple(CatalogSource(**entry) for entry in document["sources"])
