import { useEffect, useRef } from 'react';
import type { Ref } from 'react';
import { cn } from '@/lib/cn';
import { post } from './bridge';
import { ago } from './logic';
import type { Item, State } from './logic';
import { Btn, Card, Grow, Header, Hint, Input, Row, Tool } from './ui';

interface Props {
  t: (key: string, vars?: Record<string, string | number>) => string;
  state: State;
  items: Item[];
  query: string;
  selected: number;
  searchRef: Ref<HTMLInputElement>;
  onQuery: (query: string) => void;
  onSelect: (index: number) => void;
  onCopy: (id: number) => void;
  onRefresh: () => void;
}

function Body({ item, t }: { item: Item; t: Props['t'] }) {
  if (item.masked) return <Hint>{t('clip.hidden')}</Hint>;
  if (item.kind === 'image' && item.image) return <img src={`image/${item.image}`} alt={`${item.width} x ${item.height}`} className="block max-h-14 max-w-40 rounded-[5px]" />;
  if (item.kind === 'file') {
    const paths = item.preview.split('\n');
    return <div className="truncate">{paths.length > 1 ? `${paths[0]}  (${t('clip.files', { n: paths.length })})` : paths[0]}</div>;
  }
  return <div className={cn('truncate', item.kind === 'code' && 'font-mono text-xs')}>{item.preview}</div>;
}

function ItemRow({ item, index, props }: { item: Item; index: number; props: Props }) {
  const { t, selected, onCopy, onRefresh } = props;
  const ref = useRef<HTMLDivElement>(null);
  const on = index === selected;
  useEffect(() => {
    if (on) ref.current?.scrollIntoView({ block: 'nearest' });
  }, [on]);
  const when = ago(item.used, Date.now());
  return (
    <Row ref={ref} role="option" aria-selected={on} tabIndex={-1} onClick={() => onCopy(item.id)} className={cn('cursor-default hover:bg-[var(--fill)]', on && 'bg-accent-soft hover:bg-accent-soft')}>
      <span className={cn('w-14 flex-none text-[11px] tracking-[0.03em] text-fg-2 uppercase', item.pinned && 'text-accent')}>{t(`kind.${item.kind}`)}</span>
      <Grow>
        <Body item={item} t={t} />
        <Hint>{t(when.key, { n: when.n })}</Hint>
      </Grow>
      <div className="flex gap-1.5">
        <Tool
          onClick={(event) => {
            event.stopPropagation();
            post({ t: 'pin', id: item.id });
            onRefresh();
          }}
        >
          {t(item.pinned ? 'clip.unpin' : 'clip.pin')}
        </Tool>
        <Tool
          onClick={(event) => {
            event.stopPropagation();
            post({ t: 'delete', id: item.id });
            onRefresh();
          }}
        >
          {t('clip.delete')}
        </Tool>
      </div>
    </Row>
  );
}

export function ClipboardPage(props: Props) {
  const { t, state, items, query, searchRef, onQuery, onSelect, onCopy, onRefresh, selected } = props;
  const shortcut = state.config.hotkey && !state.env.hotkey_error ? <Hint>{t('clip.shortcut', { key: state.config.hotkey })}</Hint> : null;
  return (
    <>
      <Header title={t('page.clipboard')} sub={t('clip.sub')} />
      <Input
        ref={searchRef}
        id="search"
        type="search"
        className="mb-3 h-auto w-full px-3 py-2"
        placeholder={t('clip.search')}
        aria-label={t('clip.search')}
        value={query}
        onChange={(event) => onQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown') {
            onSelect(Math.min(items.length - 1, selected + 1));
            event.preventDefault();
          } else if (event.key === 'ArrowUp') {
            onSelect(Math.max(0, selected - 1));
            event.preventDefault();
          } else if (event.key === 'Enter' && items[selected]) onCopy(items[selected].id);
        }}
      />
      <Card>
        <Row>
          <Grow>
            <Hint>{t('clip.count', { n: state.env.count })}</Hint>
          </Grow>
          {shortcut}
          <Btn
            variant="danger"
            onClick={() => {
              if (confirm(t('clip.clear.ask'))) {
                post({ t: 'clear' });
                onRefresh();
              }
            }}
          >
            {t('clip.clear')}
          </Btn>
        </Row>
      </Card>
      <div className="h-4" />
      {items.length ? (
        <Card role="listbox" aria-label={t('page.clipboard')}>
          {items.map((item, index) => (
            <ItemRow key={item.id} item={item} index={index} props={props} />
          ))}
        </Card>
      ) : (
        <div className="p-[30px] text-center text-fg-2">{query ? t('clip.none') : t('clip.empty')}</div>
      )}
    </>
  );
}
