import { describe, expect, it } from 'vitest';
import { parseDraft } from '../../components/config/lineDraft';
import { whitelistEntries } from '../../components/config/whitelistLines';
import { cannedKeys, isCannedEntry, patternsMissingFromAllowlist } from './whitelistGroups';

const entries = whitelistEntries(
  parseDraft('login.example.com\nmine.example\n/^ads/\nads.*.example\n*.cdn.example'),
);

describe('whitelist groups', () => {
  it('treats lines that match the default whitelist as canned', () => {
    const keys = cannedKeys('# seeded\nLogin.Example.com\n');
    expect(entries.filter((entry) => isCannedEntry(entry, keys)).map((entry) => entry.pattern)).toEqual([
      'login.example.com',
    ]);
  });

  it('names the patterns a Pi-hole allowlist cannot express', () => {
    expect(patternsMissingFromAllowlist(entries)).toEqual(['/^ads/', 'ads.*.example']);
  });
});
