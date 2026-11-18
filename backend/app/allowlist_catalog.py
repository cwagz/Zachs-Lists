"""Public allowlist catalog seeded into the source library.

These entries use the same library as blocklists and are flagged
``list_type="allow"``. Nothing here is selected for a user until they opt in.
"""

from dataclasses import dataclass

LIST_TYPE_BLOCK = "block"
LIST_TYPE_ALLOW = "allow"
LIST_TYPES = frozenset({LIST_TYPE_BLOCK, LIST_TYPE_ALLOW})


@dataclass(frozen=True)
class AllowlistCatalogEntry:
    key: str
    name: str
    url: str
    description: str
    license: str
    project_url: str


# Descriptions are taken from each list's own header or README.
SEED_ALLOWLISTS: tuple[AllowlistCatalogEntry, ...] = (
    AllowlistCatalogEntry(
        key="hagezi-referral-native",
        name="HaGeZi Allowlist Referral",
        url=(
            "https://raw.githubusercontent.com/hagezi/dns-blocklists/main/"
            "adblock/whitelist-referral-native.txt"
        ),
        description=(
            "Unblocks affiliate and tracking referral links that show up in "
            "mail and search results. HaGeZi's stricter blocklists remove some "
            "of those links; this native Adblock allowlist puts the referral "
            "exceptions back."
        ),
        license="GPL-3.0",
        project_url="https://github.com/hagezi/dns-blocklists",
    ),
    AllowlistCatalogEntry(
        key="goodnessjson-pihole-whitelist",
        name="GoodnessJSON Pi-hole Whitelist",
        url=(
            "https://raw.githubusercontent.com/GoodnessJSON/PiHole-Whitelist/"
            "master/lists/whitelist.txt"
        ),
        description=(
            "Domains that are safe to allow and that restore features large "
            "blocklists break, such as YouTube watch history and embedded "
            "video. The list leaves out advertising and tracking domains."
        ),
        license="MIT",
        project_url="https://github.com/GoodnessJSON/PiHole-Whitelist",
    ),
)
