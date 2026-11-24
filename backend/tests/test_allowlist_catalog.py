"""Catalog seeding, subscription checks, and the save that would apply nowhere."""

import json
from pathlib import Path

from app.allowlist_catalog import NOWHERE_CODE, load_catalog
from app.allowlist_selection import (
    resolve_allowlist_selection,
    whitelist_applies_nowhere,
)
from app.models.blocklist_library import BlocklistLibrary
from app.services.allowlist_seed import ensure_allowlist_catalog
from app.utils.validators import MAX_CONFIG_SOURCES
from tests.conftest import Api


def test_catalog_seeds_only_missing_rows_and_keeps_the_url_unique(api: Api) -> None:
    ensure_allowlist_catalog()
    ensure_allowlist_catalog()
    stored = list(api.db.blocklist_library.find())
    assert len(stored) == len(load_catalog())
    edited = api.db.blocklist_library.find_one({"seed_key": "hagezi-referral-native"})
    api.db.blocklist_library.update_one(
        {"_id": edited["_id"]},
        {"$set": {"name": "Renamed by an admin", "license": "MIT"}},
    )
    ensure_allowlist_catalog()
    again = api.db.blocklist_library.find_one({"_id": edited["_id"]})
    assert again["name"] == "Renamed by an admin"
    assert again["license"] == "MIT"
    assert again["url"] == edited["url"]
    names = {
        index["name"] for index in BlocklistLibrary.get_collection().list_indexes()
    }
    assert "url_1" in names


def test_selection_uses_the_shared_catalog_and_the_source_cap() -> None:
    catalog = load_catalog()
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2]
            / "tests/fixtures/allowlist-catalog.json"
        ).read_text(encoding="utf-8")
    )
    assert [entry.url for entry in catalog] == [
        item["url"] for item in fixture["sources"]
    ]
    selected, error = resolve_allowlist_selection(
        [catalog[1].url, catalog[0].url, catalog[1].url], catalog, MAX_CONFIG_SOURCES
    )
    assert error is None
    assert [item["license"] for item in selected] == ["MIT", "GPL-3.0"]
    _none, too_many = resolve_allowlist_selection(
        ["https://example.test/extra"] * (MAX_CONFIG_SOURCES + 1),
        catalog,
        50_000,
    )
    assert too_many == f"Too many allowlist sources. Maximum: {MAX_CONFIG_SOURCES}"


def test_turning_filtering_off_without_publishing_is_rejected(api: Api) -> None:
    assert whitelist_applies_nowhere(False, False)
    assert not whitelist_applies_nowhere(True, False)
    assert not whitelist_applies_nowhere(False, True)
    client = api.client(api.account(4001, "sam"))
    refused = client.put(
        "/api/user/whitelist",
        json={"apply_whitelist_to_blocklists": False, "publish_allowlist": False},
    )
    assert refused.status_code == 400
    assert refused.get_json()["code"] == NOWHERE_CODE
    assert (
        api.db.users.find_one({"username": "sam"})
        .get("config", {})
        .get("apply_whitelist_to_blocklists")
        is None
    )

    saved = client.put(
        "/api/user/whitelist",
        json={
            "apply_whitelist_to_blocklists": False,
            "publish_allowlist": True,
            "allowlist_sources": [load_catalog()[0].url],
        },
    )
    assert saved.status_code == 200
    stored = api.db.users.find_one({"username": "sam"})["config"]
    assert stored["apply_whitelist_to_blocklists"] is False
    assert stored["publish_allowlist"] is True
    assert stored["allowlist_sources"][0]["license"] == "GPL-3.0"
    assert saved.get_json()["rebuild_recommended"] is True
