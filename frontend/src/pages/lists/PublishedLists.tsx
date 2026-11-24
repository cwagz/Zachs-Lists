import { useState } from 'react';
import type { UserListsResponse } from '../../api/lists';
import { FormatSwitch } from '../../components/lists/FormatSwitch';
import { Card, CardHeader } from '../../components/ui/Card';
import { Skeleton } from '../../components/ui/Layout';
import { UrlField } from '../../components/ui/UrlField';
import { SITE_URL } from '../../config/site';
import { formatDateTime } from '../../lib/format';
import {
  FORMAT_NAMES,
  FORMAT_SAMPLES,
  FORMAT_USES,
  SERVER_DEFAULT_FORMAT,
  orderLists,
} from '../../lib/listCatalogue';
import type { Job, ListFormat } from '../../types';
import { compressedSize } from '../../lib/builds';
import {
  ALLOWLIST_ROW,
  describeList,
  publishedListAddress,
  unavailableFormatMessage,
} from './listsFacts';

export interface PublishedListsProps {
  username: string;
  lists: UserListsResponse;
  publishedBuild: Job | null;
  now: Date;
  allowlist?: { ruleCount: number | null; publishedAt: string | null } | null;
}

export function PublishedLists({
  username,
  lists,
  publishedBuild,
  now,
  allowlist = null,
}: PublishedListsProps) {
  const [format, setFormat] = useState<ListFormat>(SERVER_DEFAULT_FORMAT);
  const ordered = orderLists(lists.lists, (list) => list.name);
  const rows = [
    ...ordered.map((list) => ({
      name: list.name,
      formats: list.formats,
      domainCount: list.domain_count,
      lastUpdated: list.last_updated,
    })),
    ...(allowlist
      ? [
          {
            name: ALLOWLIST_ROW,
            formats: ['adblock', 'plain'] as ListFormat[],
            domainCount: allowlist.ruleCount,
            lastUpdated: allowlist.publishedAt ?? '',
          },
        ]
      : []),
  ];
  const published = formatDateTime(lists.published_at, { now });
  return (
    <Card as="section" aria-labelledby="published-lists">
      <CardHeader
        title="Published lists"
        titleId="published-lists"
        actions={
          <>
            {published ? <span className="narrow:hidden">Published {published}</span> : null}
            <FormatSwitch value={format} onChange={setFormat} />
          </>
        }
      />
      <p className="border-b border-border px-[18px] py-3 text-[13px] text-text-secondary narrow:px-3">
        {FORMAT_USES[format]} A line looks like{' '}
        <code translate="no" className="mono rounded-md bg-sunken px-1.5 py-0.5 break-all">
          {FORMAT_SAMPLES[format]}
        </code>
        .
      </p>
      <ul className="m-0 list-none p-0">
        {rows.map((list) => {
          const address = publishedListAddress(
            SITE_URL,
            username,
            list.name,
            list.formats,
            format,
          );
          return (
            <li
              key={list.name}
              className="grid gap-2.5 border-b border-border px-[18px] py-3.5 last:border-b-0 narrow:px-3"
            >
              <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1">
                <h3 className="font-medium" translate="no">
                  {list.name}
                </h3>
                <span className="num text-[13px] text-text-secondary">
                  {describeList(
                    { domain_count: list.domainCount, last_updated: list.lastUpdated },
                    list.name === ALLOWLIST_ROW
                      ? null
                      : compressedSize(publishedBuild, list.name, format),
                    now,
                  )}
                </span>
              </div>
              {address ? (
                <UrlField
                  label={`Address of the ${list.name} list in the ${FORMAT_NAMES[format]} format`}
                  url={address}
                />
              ) : (
                <p className="text-[13px] text-text-tertiary">
                  {unavailableFormatMessage(list.name, format)}
                </p>
              )}
            </li>
          );
        })}
      </ul>
    </Card>
  );
}

export function PublishedListsSkeleton() {
  return (
    <Card>
      <div className="border-b border-border px-[18px] py-3.5">
        <Skeleton className="h-5 w-32" />
      </div>
      {[0, 1, 2].map((index) => (
        <div
          key={index}
          className="grid gap-2.5 border-b border-border px-[18px] py-3.5 last:border-b-0"
        >
          <Skeleton className="h-4 w-72 max-w-full" />
          <Skeleton className="h-[52px] w-full" />
        </div>
      ))}
    </Card>
  );
}
