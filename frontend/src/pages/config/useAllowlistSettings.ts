import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { adminKeys, defaultConfigQuery, updateDefaultConfig } from '../../api/admin';
import {
  configKeys,
  saveAllowlistSettings,
  whitelistQuery,
  type AllowlistReport,
  type AllowlistSourceChoice,
} from '../../api/config';
import { describeApiError } from '../../api/errors';
import { showToast } from '../../components/ui/toasts';
import type { DefaultConfig } from '../../types';

export type AllowlistSettingsTarget = 'user' | 'default';

export interface AllowlistSettingsState {
  apply: boolean;
  publish: boolean;
  sources: AllowlistSourceChoice[];
  catalog: AllowlistSourceChoice[];
  reports: AllowlistReport[];
}

function fromUser(data: {
  apply_whitelist_to_blocklists?: boolean;
  publish_allowlist: boolean;
  allowlist_sources?: AllowlistSourceChoice[];
  allowlist_reports?: AllowlistReport[];
  allowlist_catalog?: AllowlistSourceChoice[];
}): AllowlistSettingsState {
  return {
    apply: data.apply_whitelist_to_blocklists !== false,
    publish: data.publish_allowlist,
    sources: data.allowlist_sources ?? [],
    catalog: data.allowlist_catalog ?? [],
    reports: data.allowlist_reports ?? [],
  };
}

function fromDefault(data: DefaultConfig): AllowlistSettingsState {
  return {
    apply: data.apply_whitelist_to_blocklists !== false,
    publish: data.publish_allowlist,
    sources: data.allowlist_sources ?? [],
    catalog: data.allowlist_catalog ?? [],
    reports: data.allowlist_reports ?? [],
  };
}

export function useAllowlistSettings(target: AllowlistSettingsTarget) {
  const client = useQueryClient();
  const user = useQuery({ ...whitelistQuery(), enabled: target === 'user' });
  const defaults = useQuery({ ...defaultConfigQuery(), enabled: target === 'default' });
  const state =
    target === 'user'
      ? user.data
        ? fromUser(user.data)
        : null
      : defaults.data
        ? fromDefault(defaults.data)
        : null;
  const mutation = useMutation({
    mutationFn: (body: {
      publish_allowlist?: boolean;
      apply_whitelist_to_blocklists?: boolean;
      allowlist_sources?: string[];
    }) => (target === 'user' ? saveAllowlistSettings(body) : updateDefaultConfig(body)),
    onSuccess: (result) => {
      if (target === 'user') {
        client.setQueryData(
          configKeys.whitelist(),
          (current: Parameters<typeof fromUser>[0] | undefined) =>
            current
              ? {
                  ...current,
                  publish_allowlist: result.publish_allowlist,
                  apply_whitelist_to_blocklists:
                    result.apply_whitelist_to_blocklists ?? current.apply_whitelist_to_blocklists,
                  allowlist_sources: result.allowlist_sources ?? current.allowlist_sources,
                }
              : current,
        );
      } else {
        client.setQueryData<DefaultConfig>(adminKeys.defaultConfig(), (current) =>
          current
            ? {
                ...current,
                publish_allowlist: result.publish_allowlist,
                apply_whitelist_to_blocklists:
                  result.apply_whitelist_to_blocklists ?? current.apply_whitelist_to_blocklists,
                allowlist_sources: result.allowlist_sources ?? current.allowlist_sources,
              }
            : current,
        );
      }
      showToast('ok', 'Saved. Build now to fetch allowlists and refresh your lists.');
    },
  });
  return {
    state,
    pending: mutation.isPending,
    error: mutation.error ? describeApiError(mutation.error, 'Try again shortly.') : null,
    save: mutation.mutate,
  };
}
