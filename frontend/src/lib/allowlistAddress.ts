export const ALLOWLIST_NAME = 'allowlist';
export const ALLOWLIST_PRIMARY_FORMAT = 'adblock';

export type AllowlistFormat = 'adblock' | 'plain';

/** The bare URL is the Pi-hole v6 allowlist. Plain format is explicit. */
export function allowlistAddress(base: string, format?: AllowlistFormat): string {
  if (!format || format === ALLOWLIST_PRIMARY_FORMAT) return base;
  return `${base}?format=${format}`;
}

export function userAllowlistAddress(
  siteUrl: string,
  username: string,
  format?: AllowlistFormat,
): string {
  return allowlistAddress(`${siteUrl}/u/${username}/${ALLOWLIST_NAME}.txt`, format);
}

export function defaultAllowlistAddress(siteUrl: string, format?: AllowlistFormat): string {
  return allowlistAddress(`${siteUrl}/lists/${ALLOWLIST_NAME}.txt`, format);
}
