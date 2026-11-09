import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { Button, IconButton } from '@/components/macos/Controls';
import { confirmDialog } from '@/components/macos/Dialogs';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { cx } from '@/lib/text';
import { send } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { size, sum, tilde } from './format';
import type { Item } from './types';
import { setChosen, useMoleUi } from './ui';

const act = (name: string) => send({ t: 'action', name });

export interface Verb {
  verb: string;
  label: string;
  danger?: boolean;
}

export interface PickListProps {
  kind: string;
  /** The items, or null while the first scan has not come back. */
  items: Item[] | null;
  home: string;
  busy: string[];
  detail: (item: Item) => string;
  lead?: (item: Item) => ReactNode;
  /** Makes the row of a folder open when it is clicked. */
  open?: (item: Item) => void;
  /** Goes above the toolbar. */
  head?: ReactNode;
  /** A field to search the rows by name and note. */
  filter?: boolean;
  tint?: (index: number) => string;
  /** The scan this list is read by when it differs from its kind. */
  scan?: string;
  /** What the list does to its rows besides removing them, each its own button. */
  actions?: Verb[];
  ns?: string;
}

/** A list whose rows are chosen with a checkbox and removed together. */
export function PickList({ kind, items, home, busy, detail, lead, open, head, filter, tint, scan, actions, ns = 'docker' }: PickListProps) {
  const t = useT();
  const key = scan ?? kind;
  const isBusy = (k: string) => busy.includes(k);
  const working = isBusy(actions ? `${ns}_act` : 'remove');
  const chosen = useMoleUi((s) => s.chosen[kind]) ?? [];
  const lit = useMoleUi((s) => s.lit);
  const [query, setQuery] = useState('');
  const list = items ?? [];

  // A row that is gone is no longer chosen.
  useEffect(() => {
    if (items && chosen.some((p) => !items.some((i) => i.path === p))) setChosen(kind, chosen.filter((p) => items.some((i) => i.path === p)));
  }, [items]);

  const q = filter ? query.trim().toLowerCase() : '';
  const shown = list.filter((i) => !q || `${i.name} ${i.note}`.toLowerCase().includes(q));
  const picked = list.filter((i) => chosen.includes(i.path));
  const shownPicked = shown.filter((i) => chosen.includes(i.path)).length;
  const toggle = (path: string, on: boolean) => setChosen(kind, on ? [...chosen.filter((p) => p !== path), path] : chosen.filter((p) => p !== path));

  const count =
    isBusy(key) || !items
      ? t('mole.scanning')
      : picked.length
        ? t('mole.picked', { n: picked.length, size: size(sum(picked)) })
        : t('mole.total', { n: list.length, size: size(sum(list)) });

  const confirmText = () => t('mole.confirm', { n: picked.length, size: size(sum(picked)) });
  const top = Math.max(1, ...shown.map((i) => i.size));

  const removeButton = actions ? null : (
    <Button
      small
      danger
      icon="trash"
      disabled={!picked.length || working}
      onClick={() =>
        confirmDialog({ title: confirmText(), body: t(`mole.${kind}.body`), confirm: t(`mole.${kind}.remove`) }, () => act(`mole_remove:${kind}\n${picked.map((i) => i.path).join('\n')}`))
      }
    >
      {t(`mole.${kind}.remove`)}
    </Button>
  );

  return (
    <>
      <SectionTitle>{t(`mole.sec.${kind}`)}</SectionTitle>
      <Card>
        <Row title={t(`mole.${kind}.title`)} desc={t(`mole.${kind}.desc`)} />
        {head}
        <div className="mole-toolbar">
          <label className="mole-all">
            <input
              type="checkbox"
              aria-label={t('mole.pick.all')}
              ref={(el) => {
                if (el) el.indeterminate = shownPicked > 0 && shownPicked < shown.length;
              }}
              checked={shown.length > 0 && shownPicked === shown.length}
              disabled={!shown.length}
              onChange={(e) => {
                const paths = shown.map((i) => i.path);
                setChosen(kind, e.target.checked ? [...new Set([...chosen, ...paths])] : chosen.filter((p) => !paths.includes(p)));
              }}
            />
            <span className="mole-count">{count}</span>
          </label>
          {filter ? (
            <input
              type="search"
              className="text-field mole-search"
              placeholder={t('mole.search')}
              aria-label={t('mole.search')}
              spellCheck={false}
              autoComplete="off"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          ) : null}
          {(actions ?? []).map(({ verb, label, danger }) => (
            <Button
              key={verb}
              small
              danger={danger}
              disabled={!picked.length || working}
              onClick={() => {
                const go = () => act(`mole_${ns}:${verb}\n${picked.map((i) => i.path).join('\n')}`);
                if (!danger) go();
                else confirmDialog({ title: confirmText(), body: t(`mole.${ns}.${verb}.body`), confirm: t(`mole.${ns}.${label}`) }, go);
              }}
            >
              {t(`mole.${ns}.${label}`)}
            </Button>
          ))}
          {removeButton}
          <IconButton size="sm" variant="glass" icon="refresh" label={t('mole.refresh')} disabled={isBusy(key)} onClick={() => act(`mole_scan:${key}`)} />
        </div>
        <div className="mole-list" data-kind={kind}>
          {!items ? (
            <div className="empty" role="status">
              {t('mole.scanning')}
            </div>
          ) : !shown.length ? (
            isBusy(key) ? (
              <div className="empty" role="status">
                {t('mole.scanning')}
              </div>
            ) : (
              <div className="empty">{t(`mole.${kind}.empty`)}</div>
            )
          ) : (
            shown.map((item, k) => (
              <label
                key={item.path}
                className={cx('mole-item', open && item.dir && 'opens', lit === item.path && 'lit')}
                title={tilde(item.path, home)}
                data-path={item.path}
                onClick={(e) => {
                  // A folder opens wherever its row is clicked; only the checkbox chooses it.
                  if (open && item.dir && !(e.target instanceof HTMLInputElement)) {
                    e.preventDefault();
                    open(item);
                  }
                }}
              >
                <input type="checkbox" aria-label={item.name} checked={chosen.includes(item.path)} onChange={(e) => toggle(item.path, e.target.checked)} />
                {lead ? lead(item) : null}
                <span className="mole-item-text">
                  <span className="mole-item-name">{item.name}</span>
                  <span className="mole-item-note">{detail(item)}</span>
                  <span className="mole-bar">
                    <span style={{ width: `${Math.max(1, (item.size / top) * 100)}%`, ...(tint ? { background: tint(k) } : {}) }} />
                  </span>
                </span>
                <span className="mole-item-size">{size(item.size)}</span>
                {open && item.dir ? (
                  <Button
                    small
                    icon="folder"
                    aria-label={t('mole.analyze.open', { name: item.name })}
                    onClick={(e) => {
                      e.preventDefault();
                      open(item);
                    }}
                  />
                ) : null}
              </label>
            ))
          )}
        </div>
      </Card>
    </>
  );
}
