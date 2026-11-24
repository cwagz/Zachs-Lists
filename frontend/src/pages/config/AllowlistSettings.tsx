import { useState } from 'react';
import { type AllowlistReport } from '../../api/config';
import { Alert } from '../../components/ui/Alert';
import { Button } from '../../components/ui/Button';
import { Dialog } from '../../components/ui/Dialog';
import { StatusPill } from '../../components/ui/StatusPill';
import { formatInteger, pluralise } from '../../lib/format';
import { patternsMissingFromAllowlist } from './whitelistGroups';
import type { WhitelistEntry } from '../../components/config/whitelistLines';
import { useAllowlistSettings, type AllowlistSettingsTarget } from './useAllowlistSettings';

function reportFor(reports: AllowlistReport[], url: string): AllowlistReport | undefined {
  return reports.find((report) => report.url === url);
}

function FetchStatus({ report }: { report: AllowlistReport | undefined }) {
  if (!report) return <StatusPill>Not fetched yet</StatusPill>;
  if (report.warning && report.used_cache) {
    return (
      <StatusPill tone="warn" icon="fail">
        Cached copy
      </StatusPill>
    );
  }
  if (report.warning) {
    return (
      <StatusPill tone="fail" icon="fail">
        Unavailable
      </StatusPill>
    );
  }
  return (
    <StatusPill tone="ok" icon="check">
      Fetched
    </StatusPill>
  );
}

export function AllowlistSettings({
  target,
  patterns,
}: {
  target: AllowlistSettingsTarget;
  patterns: readonly WhitelistEntry[];
}) {
  const settings = useAllowlistSettings(target);
  const [picker, setPicker] = useState(false);
  const [chosen, setChosen] = useState<string[]>([]);
  if (!settings.state) return null;
  const { apply, publish, sources, catalog, reports } = settings.state;
  const omitted = patternsMissingFromAllowlist(patterns);
  const saveSources = (urls: string[]) =>
    settings.save({
      allowlist_sources: urls,
      apply_whitelist_to_blocklists: apply,
      publish_allowlist: publish,
    });

  return (
    <div className="grid gap-3 border-b border-border px-[18px] py-3.5 narrow:px-3">
      <label className="flex items-start gap-3 text-[13px]">
        <input
          type="checkbox"
          checked={apply}
          disabled={settings.pending}
          onChange={(event) =>
            settings.save({
              apply_whitelist_to_blocklists: event.target.checked,
              publish_allowlist: publish,
            })
          }
          className="mt-0.5 size-4 flex-none accent-(--accent-start)"
        />
        <span>
          <span className="font-medium">Apply whitelist to blocklists</span>
          <span className="mt-1 block text-text-secondary">
            On by default. Subscribed allowlists are included. Leave this on, or publish the
            allowlist, so the whitelist still applies somewhere.
          </span>
        </span>
      </label>
      <label className="flex items-start gap-3 text-[13px]">
        <input
          type="checkbox"
          checked={publish}
          disabled={settings.pending}
          onChange={(event) =>
            settings.save({
              publish_allowlist: event.target.checked,
              apply_whitelist_to_blocklists: apply,
            })
          }
          className="mt-0.5 size-4 flex-none accent-(--accent-start)"
        />
        <span>
          <span className="font-medium">Publish as a Pi-hole v6 allowlist</span>
          {omitted.length > 0 ? (
            <span className="mt-1 block text-text-tertiary">
              Not written to that file: {omitted.join(', ')}. Add a regular expression in Pi-hole
              with <code className="font-mono">pihole --allow-regex</code>.
            </span>
          ) : null}
        </span>
      </label>
      {settings.error ? (
        <Alert tone="fail" title="The allowlist setting was not saved">
          {settings.error}
        </Alert>
      ) : null}
      <div className="flex items-center justify-between gap-3">
        <h3 className="text-xs font-medium text-text-secondary">
          Subscriptions{' '}
          <span className="font-normal text-text-tertiary">{pluralise(sources.length, 'list')}</span>
        </h3>
        <Button size="sm" icon="plus" onClick={() => setPicker(true)} disabled={settings.pending}>
          Add allowlist source
        </Button>
      </div>
      {sources.length === 0 ? (
        <p className="text-[13px] text-text-secondary">
          No public allowlist is selected. Add one to include its domains with your whitelist.
        </p>
      ) : (
        <ul className="m-0 list-none p-0">
          {sources.map((source) => {
            const report = reportFor(reports, source.url);
            return (
              <li
                key={source.url}
                className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-t border-border py-2.5"
              >
                <div className="min-w-0">
                  <span className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-medium break-words">{source.name}</span>
                    <span className="text-xs text-text-tertiary">{source.license}</span>
                    <FetchStatus report={report} />
                  </span>
                  <span className="mt-0.5 block text-xs text-text-tertiary">
                    {report?.kept == null
                      ? 'Domain count appears after the next build'
                      : `${formatInteger(report.kept)} domains`}
                  </span>
                  {report?.warning ? (
                    <span className="mt-0.5 block text-xs break-words text-text-tertiary">
                      {report.warning}
                    </span>
                  ) : null}
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={settings.pending}
                  onClick={() => saveSources(sources.filter((item) => item.url !== source.url).map((item) => item.url))}
                  aria-label={`Remove ${source.name}`}
                >
                  Remove
                </Button>
              </li>
            );
          })}
        </ul>
      )}
      <Dialog
        open={picker}
        onClose={() => {
          setPicker(false);
          setChosen([]);
        }}
        title="Add allowlist source"
        description="Public allowlists are added to your whitelist on the next build. None are selected until you add them."
        footer={
          <>
            <Button
              variant="ghost"
              onClick={() => {
                setPicker(false);
                setChosen([]);
              }}
            >
              Cancel
            </Button>
            <Button
              variant="primary"
              disabled={chosen.length === 0 || settings.pending}
              onClick={() => {
                saveSources([...sources.map((source) => source.url), ...chosen]);
                setPicker(false);
                setChosen([]);
              }}
            >
              Add
            </Button>
          </>
        }
      >
        {catalog.length === 0 ? (
          <p className="text-[13px] text-text-secondary">No public allowlists are in the catalog.</p>
        ) : (
          <ul className="m-0 list-none p-0">
            {catalog.map((entry) => {
              const present = sources.some((source) => source.url === entry.url);
              const selected = chosen.includes(entry.url);
              return (
                <li key={entry.url} className="border-b border-border last:border-b-0">
                  <label
                    className={
                      present
                        ? 'flex gap-3 px-1 py-3 opacity-70'
                        : 'flex cursor-pointer gap-3 px-1 py-3 hover:bg-surface-hover'
                    }
                  >
                    <input
                      type="checkbox"
                      checked={present || selected}
                      disabled={present}
                      onChange={() =>
                        setChosen((current) =>
                          current.includes(entry.url)
                            ? current.filter((url) => url !== entry.url)
                            : [...current, entry.url],
                        )
                      }
                      className="mt-0.5 size-4 flex-none accent-(--accent-start)"
                    />
                    <span className="min-w-0">
                      <span className="flex flex-wrap items-center gap-2">
                        <span className="text-sm font-medium">{entry.name}</span>
                        <span className="text-xs text-text-tertiary">{entry.license}</span>
                        {present ? (
                          <StatusPill tone="ok" icon="check">
                            Subscribed
                          </StatusPill>
                        ) : null}
                      </span>
                      {entry.description ? (
                        <span className="mt-0.5 block text-[13px] text-text-secondary">
                          {entry.description}
                        </span>
                      ) : null}
                    </span>
                  </label>
                </li>
              );
            })}
          </ul>
        )}
      </Dialog>
    </div>
  );
}
