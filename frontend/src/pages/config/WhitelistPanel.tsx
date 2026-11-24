import { useQuery } from '@tanstack/react-query';
import { useMemo, useState, type FormEvent } from 'react';
import { whitelistQuery } from '../../api/config';
import { SegmentedControl } from '../../components/ui/SegmentedControl';
import {
  WHITELIST_KINDS,
  describeWhitelistMatch,
  describeWhitelistProblem,
  groupWhitelist,
  hasWhitelistEntry,
  whitelistKindLabel,
  type WhitelistEntry,
} from '../../components/config/whitelistLines';
import { Button } from '../../components/ui/Button';
import { Field, Input } from '../../components/ui/Field';
import { EmptyState } from '../../components/ui/States';
import { StatusPill } from '../../components/ui/StatusPill';
import { pluralise } from '../../lib/format';
import { cannedKeys, isCannedEntry } from './whitelistGroups';

export interface WhitelistPanelProps {
  entries: WhitelistEntry[];
  commentCount: number;
  problemLines: ReadonlySet<number>;
  onAdd: (pattern: string) => void;
  onRemove: (lineId: number) => void;
}

function AddEntryForm({
  entries,
  onAdd,
}: {
  entries: WhitelistEntry[];
  onAdd: (pattern: string) => void;
}) {
  const [pattern, setPattern] = useState('');
  const [error, setError] = useState<string | null>(null);
  const trimmed = pattern.trim();
  const preview =
    trimmed && describeWhitelistProblem(trimmed) === null ? describeWhitelistMatch(trimmed) : null;

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const problem =
      describeWhitelistProblem(trimmed) ??
      (hasWhitelistEntry(entries, trimmed) ? 'This is already in your whitelist.' : null);
    setError(problem);
    if (problem) {
      const field = event.currentTarget.elements.namedItem('pattern');
      if (field instanceof HTMLElement) field.focus();
      return;
    }
    onAdd(trimmed);
    setPattern('');
  };

  return (
    <form
      onSubmit={handleSubmit}
      noValidate
      className="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-3 border-b border-border px-[18px] py-3.5 narrow:grid-cols-[minmax(0,1fr)] narrow:px-3"
    >
      <Field
        label="Add a domain or pattern"
        error={error}
        hint={
          preview ??
          'For example example.com, @@example.com, *.example.com or a /regular expression/.'
        }
      >
        <Input
          mono
          autoComplete="off"
          autoCapitalize="off"
          spellCheck={false}
          name="pattern"
          placeholder="example.com"
          value={pattern}
          onChange={(event) => {
            setPattern(event.target.value);
            setError(null);
          }}
        />
      </Field>
      <Button type="submit" icon="plus" className="mt-[25px] narrow:mt-0">
        Add
      </Button>
    </form>
  );
}

type LineGroup = 'default' | 'yours';

export function WhitelistPanel({
  entries,
  commentCount,
  problemLines,
  onAdd,
  onRemove,
}: WhitelistPanelProps) {
  const defaults = useQuery(whitelistQuery());
  const keys = useMemo(
    () => cannedKeys(defaults.data?.default_whitelist ?? ''),
    [defaults.data?.default_whitelist],
  );
  const [group, setGroup] = useState<LineGroup>('yours');
  const [filter, setFilter] = useState('');
  const visible = entries.filter((entry) => isCannedEntry(entry, keys) === (group === 'default'));
  const needle = filter.trim().toLowerCase();
  const shown = needle
    ? visible.filter((entry) => entry.pattern.toLowerCase().includes(needle))
    : visible;
  const groups = groupWhitelist(shown);
  const cannedCount = entries.filter((entry) => isCannedEntry(entry, keys)).length;
  return (
    <div className="[&>section:last-child_li:last-child]:border-b-0">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border px-[18px] py-3 narrow:px-3">
        <SegmentedControl
          label="Whitelist lines"
          value={group}
          onChange={setGroup}
          options={[
            { value: 'default', label: `Default (${cannedCount})` },
            { value: 'yours', label: `Yours (${entries.length - cannedCount})` },
          ]}
        />
      </div>
      {group === 'yours' ? <AddEntryForm entries={entries} onAdd={onAdd} /> : null}
      {visible.length > 8 ? (
        <div className="border-b border-border px-[18px] py-3 narrow:px-3">
          <Field label="Filter lines">
            <Input
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder="example.com"
            />
          </Field>
        </div>
      ) : null}
      {shown.length === 0 ? (
        <EmptyState title={needle ? 'Nothing matches that filter' : group === 'default' ? 'No default whitelist lines' : 'No lines of your own'}>
          {needle
            ? 'Try a shorter piece of the domain.'
            : group === 'default'
              ? 'Lines that match the current default whitelist show up here. Use the default template to copy them.'
              : 'Add a domain or pattern here. It is kept with the default lines and any allowlists you subscribe to.'}
        </EmptyState>
      ) : (
        WHITELIST_KINDS.filter((kind) => groups[kind].length > 0).map((kind) => {
          const headingId = `whitelist-${kind}`;
          return (
            <section key={kind} aria-labelledby={headingId}>
              <h3
                id={headingId}
                className="flex items-center gap-2.5 border-b border-border bg-surface-raised px-[18px] py-2.5 text-xs font-medium text-text-secondary narrow:px-3"
              >
                {whitelistKindLabel(kind)}
                <span className="font-normal text-text-tertiary">
                  {pluralise(groups[kind].length, 'line')}
                </span>
              </h3>
              <ul className="m-0 list-none p-0">
                {groups[kind].map((entry) => (
                  <li
                    key={entry.line.id}
                    className="grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-x-4 border-b border-border px-[18px] py-2 [contain-intrinsic-size:auto_45px] [content-visibility:auto] hover:bg-surface-hover narrow:px-3"
                  >
                    <div className="min-w-0">
                      <span className="block font-mono text-[13px] break-all">{entry.pattern}</span>
                      {entry.note ? (
                        <span className="block text-xs break-words text-text-tertiary">
                          {entry.note}
                        </span>
                      ) : null}
                    </div>
                    <div>
                      {problemLines.has(entry.lineNumber) ? (
                        <StatusPill tone="fail" icon="fail">
                          Not valid
                        </StatusPill>
                      ) : null}
                    </div>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => onRemove(entry.line.id)}
                      aria-label={`Remove ${entry.pattern}`}
                    >
                      Remove
                    </Button>
                  </li>
                ))}
              </ul>
            </section>
          );
        })
      )}
      {commentCount > 0 ? (
        <p className="px-[18px] py-2.5 text-xs text-text-tertiary narrow:px-3">
          {pluralise(commentCount, 'comment line')} in the file {commentCount === 1 ? 'is' : 'are'}{' '}
          kept as written. Switch to Text to read or change {commentCount === 1 ? 'it' : 'them'}.
        </p>
      ) : null}
    </div>
  );
}
