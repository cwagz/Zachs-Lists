"""Public allowlist catalog shared with the worker tests.

The entries live in ``tests/fixtures/allowlist-catalog.json``. Python seeds
and stores those fields. The worker reads the stored fields and is the only
place that renders an allowlist or folds the selection into the build digest.
"""

import json
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


@dataclass(frozen=True)
class CatalogSource:
    key: str
    name: str
    url: str
    description: str
    license: str
    project_url: str


def catalog_path() -> Path:
    return (
        Path(__file__).resolve().parents[2]
        / "tests"
        / "fixtures"
        / "allowlist-catalog.json"
    )


@lru_cache(maxsize=1)
def load_catalog() -> tuple[CatalogSource, ...]:
    document = json.loads(catalog_path().read_text(encoding="utf-8"))
    return tuple(CatalogSource(**entry) for entry in document["sources"])
