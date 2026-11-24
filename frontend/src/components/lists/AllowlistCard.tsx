import type { ReactNode } from 'react';
import { Alert } from '../ui/Alert';
import { Card, CardBody, CardHeader } from '../ui/Card';
import { UrlField } from '../ui/UrlField';

export const ALLOWLIST_WARNING =
  'Like your blocklists, this allowlist is readable by anyone who can reach this instance. It contains your whitelist entries.';

export function AllowlistAddresses({
  subscriptionUrl,
  plainUrl,
}: {
  subscriptionUrl: string;
  plainUrl: string;
}) {
  return (
    <div className="grid gap-3">
      <p className="text-[13px] text-text-secondary">
        Add the first URL in Pi-hole v6 as an allow-type adlist. Plain format lists exact domains
        only and does not honor subdomain semantics.
      </p>
      <UrlField url={subscriptionUrl} label="Pi-hole v6 allowlist" />
      <UrlField url={plainUrl} label="Plain allowlist" />
    </div>
  );
}

export function AllowlistToggle({
  published,
  pending,
  error,
  onChange,
  warning = ALLOWLIST_WARNING,
  children,
}: {
  published: boolean;
  pending: boolean;
  error: string | null;
  onChange: (next: boolean) => void;
  warning?: string;
  children?: ReactNode;
}) {
  return (
    <Card as="section" aria-labelledby="publish-allowlist">
      <CardHeader title="Pi-hole v6 allowlist" titleId="publish-allowlist" />
      <CardBody className="grid gap-3">
        <label className="flex items-start gap-3 text-[13px]">
          <input
            type="checkbox"
            checked={published}
            disabled={pending}
            onChange={(event) => onChange(event.target.checked)}
            className="mt-0.5 size-4 accent-[var(--accent-start)]"
          />
          <span>
            <span className="font-medium">Publish as a Pi-hole v6 allowlist</span>
            <span className="mt-1 block text-warn">{warning}</span>
          </span>
        </label>
        {published ? children : (
          <p className="text-[13px] text-text-secondary">
            Publishing stays off until you opt in. The file uses the whitelist that is saved now.
          </p>
        )}
        {error ? (
          <Alert tone="fail" title="The allowlist setting was not saved">
            {error}
          </Alert>
        ) : null}
      </CardBody>
    </Card>
  );
}
