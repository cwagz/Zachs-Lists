import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import {
  configKeys,
  saveAllowlistPublication,
  whitelistQuery,
  type WhitelistResponse,
} from '../../api/config';
import { listsKeys } from '../../api/lists';
import { AllowlistToggle } from '../../components/lists/AllowlistCard';

export function PublishAllowlist() {
  const client = useQueryClient();
  const whitelist = useQuery(whitelistQuery());
  const [error, setError] = useState<string | null>(null);
  const mutation = useMutation({
    mutationFn: (next: boolean) => saveAllowlistPublication(next),
    onSuccess: async (result) => {
      setError(null);
      client.setQueryData<WhitelistResponse>(configKeys.whitelist(), (current) =>
        current ? { ...current, publish_allowlist: result.publish_allowlist } : current,
      );
      await client.invalidateQueries({ queryKey: listsKeys.mine() });
    },
    onError: (cause: unknown) => {
      setError(cause instanceof Error ? cause.message : 'Try again shortly.');
    },
  });

  if (!whitelist.data) return null;
  return (
    <AllowlistToggle
      published={whitelist.data.publish_allowlist}
      pending={mutation.isPending}
      error={error}
      onChange={(next) => mutation.mutate(next)}
    />
  );
}
