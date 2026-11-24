import { describe, expect, it } from 'vitest';
import {
  defaultAllowlistAddress,
  userAllowlistAddress,
} from './allowlistAddress';

const SITE = 'https://lists.example.test';

describe('allowlist addresses', () => {
  it('uses a bare URL for the Pi-hole v6 subscription', () => {
    expect(userAllowlistAddress(SITE, 'sam')).toBe(`${SITE}/u/sam/allowlist.txt`);
    expect(userAllowlistAddress(SITE, 'sam', 'adblock')).toBe(`${SITE}/u/sam/allowlist.txt`);
    expect(defaultAllowlistAddress(SITE)).toBe(`${SITE}/lists/allowlist.txt`);
  });

  it('adds a query only for the plain format', () => {
    expect(userAllowlistAddress(SITE, 'sam', 'plain')).toBe(
      `${SITE}/u/sam/allowlist.txt?format=plain`,
    );
    expect(defaultAllowlistAddress(SITE, 'plain')).toBe(
      `${SITE}/lists/allowlist.txt?format=plain`,
    );
  });
});
