import { userAllowlistAddress } from '../../lib/allowlistAddress';
import { formatBytes, formatDateTime, pluralise } from '../../lib/format';
import {
  FORMAT_NAMES,
  exactDomains,
  offersFormat,
  userListAddress,
} from '../../lib/listCatalogue';
import type { AnalyticsTotals, ListFormat, UserList } from '../../types';

export const ALLOWLIST_ROW = 'allowlist';

export function publishedListAddress(
  siteUrl: string,
  username: string,
  name: string,
  formats: ListFormat[] | undefined,
  format: ListFormat,
): string | null {
  if (name === ALLOWLIST_ROW) {
    if (format === 'hosts') return null;
    return userAllowlistAddress(siteUrl, username, format === 'plain' ? 'plain' : undefined);
  }
  if (!offersFormat(formats, format)) return null;
  return userListAddress(siteUrl, username, name, format);
}

export function unavailableFormatMessage(name: string, format: ListFormat): string {
  if (name === ALLOWLIST_ROW && format === 'hosts') {
    return 'Hosts format is not available for allowlists. Choose Adblock for @@ rules, or Plain for exact domains.';
  }
  return `This list is not published in the ${FORMAT_NAMES[format]} format.`;
}

export function isPublicList(list: Pick<UserList, 'is_public'>): boolean {
  return list.is_public !== false;
}

export function describeList(
  list: { domain_count: number | null; last_updated: string },
  compressedBytes: number | null,
  now: Date,
): string {
  const parts = [`${exactDomains(list.domain_count)} domains`];
  if (compressedBytes !== null) parts.push(`${formatBytes(compressedBytes)} compressed`);
  const published = formatDateTime(list.last_updated, { now });
  if (published) parts.push(`published ${published}`);
  return parts.join(' · ');
}

export function describeDownloads(totals: AnalyticsTotals, periodDays: number): string {
  const period = `the last ${pluralise(periodDays, 'day')}`;
  if (totals.requests === 0) return `None of your lists were downloaded in ${period}.`;
  return `Your lists were downloaded ${pluralise(totals.requests, 'time')} in ${period}, ${formatBytes(totals.bandwidth_bytes)} in total. The figure covers all of your lists together.`;
}
