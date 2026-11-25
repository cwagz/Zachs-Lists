"""Old blocklist URLs replaced or removed in stored configs.

This module is the only copy of that map. The seed file stores the new URLs.
`backend/scripts/update_source_urls.py` reads the map from here.
"""

from typing import Final

_JSDELIVR = "https://cdn.jsdelivr.net/gh/hagezi/dns-blocklists@latest"
_HAGEZI_RAW = "https://raw.githubusercontent.com/hagezi/dns-blocklists/main"
_BLP = "https://blocklistproject.github.io/Lists"
_URLHAUS_AGH = "https://malware-filter.gitlab.io/malware-filter/urlhaus-filter-agh.txt"

_HAGEZI_JSDELIVR = ("multi", "pro", "pro.plus", "ultimate", "light", "tif")
_HAGEZI_NATIVE = (
    "amazon",
    "apple",
    "huawei",
    "lgwebos",
    "roku",
    "samsung",
    "tiktok",
    "winoffice",
    "xiaomi",
)
_BLP_LISTS = (
    "abuse",
    "ads",
    "crypto",
    "drugs",
    "facebook",
    "fraud",
    "gambling",
    "malware",
    "phishing",
    "piracy",
    "porn",
    "ransomware",
    "redirect",
    "scam",
    "tiktok",
    "torrent",
    "tracking",
    "youtube",
)


def _replacements() -> dict[str, str]:
    pairs: dict[str, str] = {}
    for name in _HAGEZI_JSDELIVR:
        pairs[f"{_JSDELIVR}/hosts/{name}.txt"] = f"{_JSDELIVR}/adblock/{name}.txt"
    for name in _HAGEZI_NATIVE:
        pairs[f"{_HAGEZI_RAW}/domains/native.{name}.txt"] = (
            f"{_HAGEZI_RAW}/adblock/native.{name}.txt"
        )
    for name in _BLP_LISTS:
        pairs[f"{_BLP}/{name}.txt"] = f"{_BLP}/adguard/{name}-ags.txt"
    pairs.update(
        {
            "https://badmojr.github.io/1Hosts/Lite/hosts.txt": (
                "https://badmojr.github.io/1Hosts/Lite/adblock.txt"
            ),
            "https://raw.githubusercontent.com/jerryn70/GoodbyeAds/master/Hosts/GoodbyeAds.txt": (
                "https://raw.githubusercontent.com/jerryn70/GoodbyeAds/master/Formats/GoodbyeAds-AdBlock-Filter.txt"
            ),
            "https://raw.githubusercontent.com/notracking/hosts-blocklists/master/hostnames.txt": (
                "https://raw.githubusercontent.com/notracking/hosts-blocklists/master/adblock/adblock.txt"
            ),
            "https://raw.githubusercontent.com/Spam404/lists/master/main-blacklist.txt": (
                "https://raw.githubusercontent.com/Spam404/lists/master/adblock-list.txt"
            ),
            "https://raw.githubusercontent.com/bongochong/CombinedPrivacyBlockLists/master/newhosts-final.hosts": (
                "https://raw.githubusercontent.com/bongochong/CombinedPrivacyBlockLists/master/cpbl-abp-list.txt"
            ),
            "https://raw.githubusercontent.com/nickspaargaren/no-amazon/master/amazon.txt": (
                "https://raw.githubusercontent.com/nickspaargaren/no-amazon/master/amazon-adguard.txt"
            ),
            "https://raw.githubusercontent.com/nickspaargaren/no-google/master/pihole-google.txt": (
                "https://raw.githubusercontent.com/nickspaargaren/no-google/master/pihole-google-adguard.txt"
            ),
            "https://raw.githubusercontent.com/Perflyst/PiHoleBlocklist/master/SmartTV.txt": (
                "https://raw.githubusercontent.com/Perflyst/PiHoleBlocklist/master/SmartTV-AGH.txt"
            ),
            "https://raw.githubusercontent.com/DandelionSprout/adfilt/master/Alternate%20versions%20Anti-Malware%20List/AntiMalwareHosts.txt": (
                "https://raw.githubusercontent.com/DandelionSprout/adfilt/master/Alternate%20versions%20Anti-Malware%20List/AntiMalwareAdGuardHome.txt"
            ),
            "https://hole.cert.pl/domains/domains_hosts.txt": (
                "https://hole.cert.pl/domains/v2/domains_adblock.txt"
            ),
            "https://malware-filter.gitlab.io/malware-filter/phishing-filter-hosts.txt": (
                "https://malware-filter.gitlab.io/malware-filter/phishing-filter-agh.txt"
            ),
            "https://malware-filter.gitlab.io/malware-filter/urlhaus-filter-hosts.txt": (
                _URLHAUS_AGH
            ),
            "https://urlhaus.abuse.ch/downloads/hostfile/": _URLHAUS_AGH,
            "https://urlhaus.abuse.ch/downloads/hostfile": _URLHAUS_AGH,
            "https://phishing.army/download/phishing_army_blocklist_extended.txt": (
                "https://phishing.army/download/phishing-army-adblock.txt"
            ),
            "https://pgl.yoyo.org/adservers/serverlist.php?hostformat=hosts&showintro=0&mimetype=plaintext": (
                "https://pgl.yoyo.org/adservers/serverlist.php?hostformat=adblockplus&showintro=0&mimetype=plaintext"
            ),
        }
    )
    return pairs


REPLACEMENTS: Final[dict[str, str]] = _replacements()

# Gone upstream, with no list from the same maintainer to point at.
REMOVALS: Final[frozenset[str]] = frozenset(
    {
        "https://raw.githubusercontent.com/nextdns/cname-cloaking-blocklist/master/domains",
        "https://raw.githubusercontent.com/columndeeply/hosts/main/hosts00",
    }
)
