import { useEffect, useState } from 'react';
import { Button, IconButton, Segmented, Select, ToggleRow } from '@/components/macos/Controls';
import { confirmDialog } from '@/components/macos/Dialogs';
import { Card, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { Icon } from '@/components/macos/Icon';
import { keysText } from '@/lib/shortcuts';
import { change, send } from '@/stores/app';
import { useSetting, useT } from '@/stores/hooks';
import type { TFunction } from '@/stores/hooks';
import './stack.css';

// Clipboard extras: Copy Stack, pinned boards and sensitive data protection. Cards at the end of the Clipboard page;
// the state is under `stack`, and every change is checked again by the app.

const KEYS = ['option', 'command', 'control', 'shift'];
const CHOICES = ['off', 'mask', 'never'];
const MINUTES = [1, 5, 10, 30, 60, 240, 1440];

interface Board {
  id: number;
  name: string;
  items: number;
}

function minutesLabel(t: TFunction, n: number): string {
  if (n >= 1440 && n % 1440 === 0) return n === 1440 ? t('stack.expire.day') : t('stack.expire.min', { n });
  if (n >= 60 && n % 60 === 0) return t('stack.expire.hour', { n: n / 60 });
  return t('stack.expire.min', { n });
}

function CopyCards() {
  const t = useT();
  const enabled = useSetting<boolean>('stack.enabled');
  const accessibility = useSetting<boolean>('stack.accessibility');
  const active = useSetting<boolean>('stack.active');
  const queued = useSetting<number>('stack.queued');
  const modifier = useSetting<string>('stack.modifier');
  const status = !accessibility ? t('stack.status.needs') : active ? t('stack.status.running', { n: queued }) : t('stack.status.ready');
  return (
    <>
      <SectionTitle>{t('stack.sec.copy')}</SectionTitle>
      <Card>
        <ToggleRow path="stack.enabled" title={t('stack.copy.title')} desc={t('stack.copy.desc')} />
        <Row title={t('stack.key.title')} desc={t('stack.key.desc')} when={enabled}>
          <Segmented label={t('stack.key.title')} value={modifier} options={KEYS.map((k) => ({ value: k, label: t(`stack.key.${k}`) }))} onPick={(v) => change('stack.modifier', v)} />
        </Row>
        <Row title={t('stack.status.title')} desc={t('stack.status.desc')}>
          <Pill good={accessibility}>{status}</Pill>
          {accessibility ? null : (
            <Button small onClick={() => send({ t: 'action', name: 'open_accessibility' })}>
              {t('stack.status.open')}
            </Button>
          )}
        </Row>
      </Card>
    </>
  );
}

function BoardRow({ board, max }: { board: Board; max: number }) {
  const t = useT();
  const [name, setName] = useState(board.name);
  useEffect(() => setName(board.name), [board.name]);
  const commit = () => {
    const next = name.trim();
    if (next !== board.name) send({ t: 'action', name: `stack_board_rename:${board.id}:${next}` });
    // The list redraws with the new name when the app accepts it; until then the old one stays.
    setName(board.name);
  };
  return (
    <div className="stack-board">
      <input
        className="stack-input"
        type="text"
        value={name}
        maxLength={max}
        spellCheck={false}
        aria-label={`${t('stack.boards.rename')}: ${board.name}`}
        onChange={(e) => setName(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') e.currentTarget.blur();
        }}
      />
      <span className="stack-count">{t('stack.boards.items', { n: board.items })}</span>
      <IconButton
        icon="trash"
        label={`${t('stack.boards.remove')}: ${board.name}`}
        onClick={() =>
          confirmDialog({ title: t('stack.boards.confirm.title'), body: t('stack.boards.confirm.body'), confirm: t('stack.boards.remove') }, () =>
            send({ t: 'action', name: `stack_board_delete:${board.id}` }),
          )
        }
      />
    </div>
  );
}

function AddBoardRow({ count, limit, max }: { count: number; limit: number; max: number }) {
  const t = useT();
  const [name, setName] = useState('');
  const add = () => {
    const next = name.trim();
    if (!next) return;
    send({ t: 'action', name: `stack_board_add:${next}` });
    setName('');
  };
  return (
    <div className="row">
      <div className="row-text">
        <input
          className="stack-input"
          type="text"
          value={name}
          maxLength={max}
          spellCheck={false}
          placeholder={t('stack.boards.new')}
          aria-label={t('stack.boards.new')}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') add();
          }}
        />
      </div>
      <div className="row-control">
        <span className="stack-count">{t('stack.boards.count', { n: count, max: limit })}</span>
        <Button small disabled={count >= limit} onClick={add}>
          <Icon name="plus" />
          {t('stack.boards.add')}
        </Button>
      </div>
    </div>
  );
}

function BoardCards() {
  const t = useT();
  const boards = useSetting<Board[]>('stack.boards');
  const maxBoards = useSetting<number>('stack.max_boards');
  const maxName = useSetting<number>('stack.max_name');
  const boardKey = useSetting<string>('keys.shortcuts.board');
  return (
    <>
      <SectionTitle>{t('stack.sec.boards')}</SectionTitle>
      <Card>
        <Row title={t('stack.boards.title')} desc={t('stack.boards.desc', { key: keysText(boardKey) })} />
        <div className="apps">
          {boards.length ? boards.map((b) => <BoardRow key={b.id} board={b} max={maxName} />) : <div className="empty">{t('stack.boards.empty')}</div>}
        </div>
        <AddBoardRow count={boards.length} limit={maxBoards} max={maxName} />
      </Card>
    </>
  );
}

function SensitiveCards() {
  const t = useT();
  const sensitive = useSetting<string>('stack.sensitive');
  const expire = useSetting<number>('stack.expire_minutes');
  const minutes = MINUTES.includes(expire) ? MINUTES : [...MINUTES, expire].sort((a, b) => a - b);
  return (
    <>
      <SectionTitle>{t('stack.sec.sensitive')}</SectionTitle>
      <Card>
        <Row title={t('stack.sensitive.title')} desc={t('stack.sensitive.desc')} block>
          <Segmented label={t('stack.sensitive.title')} value={sensitive} options={CHOICES.map((c) => ({ value: c, label: t(`stack.sensitive.${c}`) }))} onPick={(v) => change('stack.sensitive', v)} />
        </Row>
        <Row title={t('stack.expire.title')} desc={t('stack.expire.desc')} when={sensitive === 'mask'}>
          <Select label={t('stack.expire.title')} value={expire} options={minutes.map((m) => ({ value: m, label: minutesLabel(t, m) }))} onChange={(v) => change('stack.expire_minutes', v)} />
        </Row>
        <Row title={t('stack.hidden.title')} desc={t('stack.hidden.desc')} />
      </Card>
    </>
  );
}

export function StackCards() {
  return (
    <>
      <CopyCards />
      <BoardCards />
      <SensitiveCards />
    </>
  );
}
