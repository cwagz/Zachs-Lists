import { parseDraft } from '../../components/config/lineDraft';
import {
  whitelistEntries,
  type WhitelistEntry,
  type WhitelistKind,
} from '../../components/config/whitelistLines';

export function entriesFromWhitelist(whitelist: string): WhitelistEntry[] {
  return whitelistEntries(parseDraft(whitelist));
}

function patternKey(pattern: string, kind: WhitelistKind): string {
  return kind === 'regex' ? pattern : pattern.toLowerCase();
}

export function cannedKeys(defaultWhitelist: string): Set<string> {
  const keys = new Set<string>();
  for (const entry of whitelistEntries(parseDraft(defaultWhitelist))) {
    keys.add(patternKey(entry.pattern, entry.entryKind));
  }
  return keys;
}

export function isCannedEntry(entry: WhitelistEntry, keys: ReadonlySet<string>): boolean {
  return keys.has(patternKey(entry.pattern, entry.entryKind));
}

/** Lines a Pi-hole v6 allowlist cannot emit. `*.example.com` can, as `@@||example.com^`. */
export function patternsMissingFromAllowlist(entries: readonly WhitelistEntry[]): string[] {
  return entries
    .filter((entry) => {
      if (entry.entryKind === 'regex') return true;
      if (entry.entryKind !== 'wildcard') return false;
      const rest = entry.pattern.slice(2);
      return !(entry.pattern.startsWith('*.') && !rest.includes('*'));
    })
    .map((entry) => entry.pattern);
}
