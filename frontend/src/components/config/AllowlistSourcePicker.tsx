import { useMemo, useState } from 'react';
import { LibraryEntry } from './types';

interface AllowlistSourcePickerProps {
  entries: LibraryEntry[];
  selectedUrls: string[];
  onSave: (urls: string[]) => Promise<void>;
  saving?: boolean;
}

export default function AllowlistSourcePicker({
  entries,
  selectedUrls,
  onSave,
  saving = false,
}: AllowlistSourcePickerProps) {
  const [chosen, setChosen] = useState<string[]>(selectedUrls);
  const [saved, setSaved] = useState<string[]>(selectedUrls);

  const dirty = useMemo(() => {
    if (chosen.length !== saved.length) return true;
    return chosen.some((url, index) => url !== saved[index]);
  }, [chosen, saved]);

  const toggle = (url: string) => {
    setChosen((current) =>
      current.includes(url) ? current.filter((item) => item !== url) : [...current, url],
    );
  };

  const handleSave = async () => {
    try {
      await onSave(chosen);
      setSaved(chosen);
    } catch {
      // The parent reports the save error.
    }
  };

  return (
    <div className="space-y-4 border-t border-pihole-border pt-6">
      <div>
        <h2 className="font-semibold text-pihole-text">Allowlist sources</h2>
        <p className="text-sm text-pihole-text-muted mt-1">
          Optional public allowlists merged into your Pi-hole v6 allowlist on the next build. None
          are selected until you opt in. Subscribed sources are fetched with the same cache as
          blocklists.
        </p>
      </div>
      {entries.length === 0 ? (
        <p className="text-sm text-pihole-text-muted">No public allowlists are in the library.</p>
      ) : (
        <div className="space-y-3">
          {entries.map((entry) => {
            const checked = chosen.includes(entry.url);
            return (
              <label
                key={entry.url}
                className="flex items-start gap-3 rounded-lg bg-pihole-dark p-3 text-sm text-pihole-text cursor-pointer"
              >
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={() => toggle(entry.url)}
                  className="mt-1 rounded border-pihole-border bg-pihole-dark text-pihole-accent focus:ring-pihole-accent"
                />
                <span className="min-w-0">
                  <span className="font-medium">{entry.name}</span>
                  {entry.license && (
                    <span className="ml-2 text-xs text-pihole-text-muted">{entry.license}</span>
                  )}
                  {entry.description && (
                    <span className="block text-pihole-text-muted mt-1">{entry.description}</span>
                  )}
                  <span className="block text-xs text-pihole-text-muted mt-1 truncate">
                    {entry.url}
                  </span>
                  {entry.project_url && (
                    <a
                      href={entry.project_url}
                      target="_blank"
                      rel="noreferrer"
                      className="inline-block text-xs text-pihole-accent mt-1"
                      onClick={(event) => event.stopPropagation()}
                    >
                      Project
                    </a>
                  )}
                </span>
              </label>
            );
          })}
        </div>
      )}
      <div className="flex justify-end">
        <button
          type="button"
          onClick={handleSave}
          disabled={!dirty || saving || entries.length === 0}
          className="btn btn-primary text-sm"
        >
          {saving ? 'Saving...' : 'Save allowlist sources'}
        </button>
      </div>
    </div>
  );
}
