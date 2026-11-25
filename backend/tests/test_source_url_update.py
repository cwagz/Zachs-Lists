import importlib.util
import re
from pathlib import Path
from typing import Any

import mongomock
from app.source_urls import REMOVALS, REPLACEMENTS

ROOT = Path(__file__).resolve().parents[2]
SEED = ROOT / "mongo-init" / "seed_library.js"
SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "update_source_urls.py"

spec = importlib.util.spec_from_file_location("update_source_urls", SCRIPT)
assert spec is not None and spec.loader is not None
script = importlib.util.module_from_spec(spec)
spec.loader.exec_module(script)

HAGEZI_PRO = "https://cdn.jsdelivr.net/gh/hagezi/dns-blocklists@latest/hosts/pro.txt"
HAGEZI_PRO_NEW = REPLACEMENTS[HAGEZI_PRO]
URLHAUS = "https://urlhaus.abuse.ch/downloads/hostfile/"
URLHAUS_HOSTS = (
    "https://malware-filter.gitlab.io/malware-filter/urlhaus-filter-hosts.txt"
)
NEXTDNS = (
    "https://raw.githubusercontent.com/nextdns/cname-cloaking-blocklist/master/domains"
)
COLUMN = "https://raw.githubusercontent.com/columndeeply/hosts/main/hosts00"
KEPT = "https://adaway.org/hosts.txt"


def _library_entries(text: str) -> list[tuple[str, str, int]]:
    entries = []
    for line in text.splitlines():
        match = re.search(
            r'name: "(?P<name>[^"]+)", url: "(?P<url>[^"]+)".*domain_count: (?P<count>\d+)',
            line,
        )
        if match:
            entries.append(
                (match.group("name"), match.group("url"), int(match.group("count")))
            )
    return entries


def _template(text: str) -> str:
    match = re.search(r'"blocklists": "(?P<body>(?:\\.|[^"\\])*)"', text)
    assert match is not None
    return match.group("body").encode("utf-8").decode("unicode_escape")


def test_map_has_no_chains_or_dead_targets() -> None:
    assert REMOVALS.isdisjoint(REPLACEMENTS)
    assert set(REPLACEMENTS).isdisjoint(REPLACEMENTS.values())
    assert all(url.startswith("https://") for url in REPLACEMENTS.values())


def test_seed_matches_the_url_map() -> None:
    text = SEED.read_text()
    entries = _library_entries(text)
    urls = [url for _name, url, _count in entries]
    assert len(urls) == len(set(urls))
    template = _template(text)
    combined = text + "\n" + template
    for old in list(REPLACEMENTS) + list(REMOVALS):
        assert old not in combined
    for new in REPLACEMENTS.values():
        assert new in combined
    targets = set(REPLACEMENTS.values())
    for name, url, count in entries:
        if url in targets:
            assert count == 0, name
    names = {name for name, _url, _count in entries}
    assert "URLhaus Malware" not in names
    assert "ColumnDeeply Porn" not in names
    assert "NextDNS CNAME Cloaking" not in names
    assert "Curbengh URLhaus Filter" in names
    assert "Hagezi Pro" in names
    assert "Phishing Army Basic" in names
    assert "Phishing Army Extended" in names
    by_name = {name: url for name, url, _count in entries}
    assert by_name["Curbengh URLhaus Filter"] == REPLACEMENTS[URLHAUS]
    assert (
        by_name["Phishing Army Extended"]
        == REPLACEMENTS[
            "https://phishing.army/download/phishing_army_blocklist_extended.txt"
        ]
    )
    assert (
        by_name["Phishing Army Basic"]
        == "https://phishing.army/download/phishing_army_blocklist.txt"
    )
    assert f"{REPLACEMENTS[URLHAUS]}|urlhaus|malicious" in template
    assert "nextdns_cname" not in template
    assert "hostformat=adblockplus" in template
    assert "hostformat=hosts" not in template


def test_rewrite_preserves_suffix_comments_and_line_endings() -> None:
    original = (
        "# note https://cdn.jsdelivr.net/gh/hagezi/dns-blocklists@latest/hosts/pro.txt\r\n"
        f"  {HAGEZI_PRO}|hagezi_pro|comprehensive|abp\r\n"
        f"{NEXTDNS}|nextdns_cname|tracking\r\n"
        f"{KEPT}|adaway|advertising\n"
    )
    updated, edits = script.rewrite_config(original)
    assert edits == [
        ("rewrite", HAGEZI_PRO, HAGEZI_PRO_NEW),
        ("remove", NEXTDNS, ""),
    ]
    assert updated == (
        "# note https://cdn.jsdelivr.net/gh/hagezi/dns-blocklists@latest/hosts/pro.txt\r\n"
        f"  {HAGEZI_PRO_NEW}|hagezi_pro|comprehensive|abp\r\n"
        f"{KEPT}|adaway|advertising\n"
    )
    assert script.rewrite_config(updated) == (updated, [])


def _database() -> Any:
    database: Any = mongomock.MongoClient().default
    database[script.LIBRARY].insert_many(
        [
            {
                "_id": "pro",
                "name": "Hagezi Pro",
                "url": HAGEZI_PRO,
                "category": "comprehensive",
                "domain_count": 10,
            },
            {
                "_id": "abuse",
                "name": "URLhaus Malware",
                "url": URLHAUS,
                "category": "malicious",
                "domain_count": 5,
            },
            {
                "_id": "curb",
                "name": "Curbengh URLhaus Filter",
                "url": URLHAUS_HOSTS,
                "category": "malicious",
                "domain_count": 8,
            },
            {
                "_id": "gone",
                "name": "ColumnDeeply Porn",
                "url": COLUMN,
                "category": "nsfw",
                "domain_count": 0,
            },
            {
                "_id": "kept",
                "name": "AdAway",
                "url": KEPT,
                "category": "advertising",
                "domain_count": 4,
            },
        ]
    )
    blocklists = (
        f"# keep\n{HAGEZI_PRO}|hagezi_pro|comprehensive\n"
        f"{URLHAUS}|urlhaus|malicious\n"
        f"{NEXTDNS}|nextdns_cname|tracking\n"
        f"{KEPT}|adaway|advertising\n"
    )
    database[script.SYSTEM_CONFIG].insert_one(
        {"_id": script.DEFAULT_CONFIG_ID, "blocklists": blocklists}
    )
    database[script.USERS].insert_many(
        [
            {
                "_id": "user-a",
                "username": "ada",
                "config": {"blocklists": blocklists, "revision": 2},
            },
            {
                "_id": "user-b",
                "username": "bea",
                "config": {"blocklists": f"{KEPT}|adaway|advertising\n", "revision": 1},
            },
        ]
    )
    return database


def test_dry_run_then_apply_is_idempotent() -> None:
    database = _database()
    before = database[script.LIBRARY].find_one({"_id": "pro"})["url"]
    report = script.run(database)
    rendered = script.render(report["changes"], report["applied"])
    assert report["applied"] is False
    assert report["rewrite"] > 0
    assert report["remove"] > 0
    assert "[blocklist_library] Hagezi Pro: rewrite" in rendered
    assert HAGEZI_PRO in rendered
    assert HAGEZI_PRO_NEW in rendered
    assert "[blocklist_library] ColumnDeeply Porn: remove" in rendered
    assert "[blocklist_library] URLhaus Malware: remove" in rendered
    assert "duplicate of Curbengh URLhaus Filter" in rendered
    assert "[users] ada: rewrite" in rendered
    assert "[users] ada: remove" in rendered
    assert "[system_config] default_config: rewrite" in rendered
    assert "bea" not in rendered
    assert database[script.LIBRARY].find_one({"_id": "pro"})["url"] == before

    applied = script.run(database, apply=True)
    assert applied["applied"] is True
    urls = {
        doc["name"]: doc["url"]
        for doc in database[script.LIBRARY].find({}, {"name": 1, "url": 1})
    }
    assert urls["Hagezi Pro"] == HAGEZI_PRO_NEW
    assert urls["Curbengh URLhaus Filter"] == REPLACEMENTS[URLHAUS]
    assert "URLhaus Malware" not in urls
    assert "ColumnDeeply Porn" not in urls
    assert urls["AdAway"] == KEPT
    assert (
        database[script.LIBRARY].find_one({"name": "Hagezi Pro"})["domain_count"] == 0
    )
    assert database[script.LIBRARY].find_one({"name": "AdAway"})["domain_count"] == 4
    stored = database[script.USERS].find_one({"_id": "user-a"})["config"]
    assert stored["revision"] == 3
    assert f"{HAGEZI_PRO_NEW}|hagezi_pro|comprehensive" in stored["blocklists"]
    assert f"{REPLACEMENTS[URLHAUS]}|urlhaus|malicious" in stored["blocklists"]
    assert "nextdns" not in stored["blocklists"]
    assert stored["blocklists"].startswith("# keep\n")
    default = database[script.SYSTEM_CONFIG].find_one()["blocklists"]
    assert f"{HAGEZI_PRO_NEW}|hagezi_pro|comprehensive" in default
    assert "nextdns" not in default
    assert database[script.USERS].find_one({"_id": "user-b"})["config"]["revision"] == 1

    again = script.run(database)
    assert again["changes"] == []
    assert script.render(again["changes"], False) == "Dry run. No changes.\n"
    assert script.run(database, apply=True)["changes"] == []
