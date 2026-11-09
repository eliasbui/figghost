import { useEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';
import { Button, IconButton, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { confirmDialog } from '@/components/macos/Dialogs';
import { Card, InfoRow, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { InfoTip } from '@/components/macos/Overlay';
import { cx } from '@/lib/text';
import { send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './snippets.css';

interface Item {
  abbr: string;
  text: string;
  shadowed_by?: string;
}
interface SnippetsState {
  enabled: boolean;
  other?: string;
  accessibility: boolean;
  items: Item[];
  max_abbr: number;
  max_text: number;
  max_items: number;
}

const count = (text: string) => [...text].length;

/** What is wrong with the abbreviation and the text, by the same rules as the app's, so that the person sees the reason at
 *  once; the app checks again. An empty string is nothing wrong. */
function problemOf(s: SnippetsState, abbr: string, text: string, editing: string | null): string {
  if (!abbr) return 'empty';
  if (count(abbr) > s.max_abbr) return 'long';
  if (/[\s\u0000-\u001f\u007f]/.test(abbr)) return 'space';
  if (!/^[!-~]+$/.test(abbr)) return 'untypeable';
  if (s.items.some((i) => i.abbr === abbr && i.abbr !== editing)) return 'duplicate';
  if (editing === null && s.items.length >= s.max_items) return 'full';
  if (!text) return 'no_text';
  if (count(text) > s.max_text) return 'text_long';
  return '';
}

function preview(text: string): string {
  const joined = text
    .split('\n')
    .filter((l) => l.trim())
    .slice(0, 2)
    .join(' / ');
  return joined.length > 140 ? `${joined.slice(0, 140)}...` : joined;
}

/** The form adds a snippet, or changes the one being edited. */
function Form({ s, on, editing, fields, setEditing, setFields, abbrField }: {
  s: SnippetsState;
  on: boolean;
  editing: string | null;
  fields: { abbr: string; text: string };
  setEditing: (abbr: string | null) => void;
  setFields: (fields: { abbr: string; text: string }) => void;
  abbrField: RefObject<HTMLInputElement | null>;
}) {
  const t = useT();
  const pending = useRef<{ abbr: string; text: string } | null>(null);
  const problem = problemOf(s, fields.abbr, fields.text, editing);
  const started = !!(fields.abbr || fields.text);
  const message = problem && started && !(problem === 'no_text' && !fields.text && !fields.abbr) ? t(`fault.snippets_${problem}`) : '';

  const clear = () => {
    pending.current = null;
    setEditing(null);
    setFields({ abbr: '', text: '' });
  };
  // The edit is done once the app's list holds what was sent.
  useEffect(() => {
    const sent = pending.current;
    if (sent && s.items.some((i) => i.abbr === sent.abbr && i.text === sent.text)) clear();
  }, [s.items]);

  const submit = () => {
    if (problem) return;
    const value = { abbr: fields.abbr, text: fields.text };
    pending.current = value;
    if (editing === null) send({ t: 'set', path: 'snippets.add', value });
    else send({ t: 'set', path: 'snippets.update', value: { ...value, from: editing } });
  };

  return (
    <div className={cx('snip-form', !on && 'disabled')}>
      <label className="snip-label">
        <span className="snip-caption">
          {t('snippets.abbr')}
          <InfoTip>{t('snippets.abbr.hint')}</InfoTip>
        </span>
        <input
          ref={abbrField}
          type="text"
          className="text-field snip-abbr"
          placeholder={t('snippets.abbr.ph')}
          aria-label={t('snippets.abbr')}
          spellCheck={false}
          autoComplete="off"
          autoCapitalize="off"
          autoCorrect="off"
          value={fields.abbr}
          onChange={(e) => setFields({ ...fields, abbr: e.target.value })}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              submit();
            }
          }}
        />
      </label>
      <label className="snip-label">
        {t('snippets.text')}
        <textarea className="snip-text" rows={5} placeholder={t('snippets.text.ph')} aria-label={t('snippets.text')} spellCheck={false} value={fields.text} onChange={(e) => setFields({ ...fields, text: e.target.value })} />
      </label>
      <div className="row-desc snip-error" role="alert">
        {message}
      </div>
      <div className="snip-actions">
        <Button primary small disabled={!!problem} onClick={submit}>
          {editing === null ? t('snippets.add') : t('snippets.save')}
        </Button>
        {editing === null ? null : (
          <Button small onClick={clear}>
            {t('cancel')}
          </Button>
        )}
      </div>
    </div>
  );
}

function ItemRow({ item, onEdit }: { item: Item; onEdit: () => void }) {
  const t = useT();
  const lines = item.text.split('\n').length;
  const meta = lines > 1 ? t('snippets.lines', { n: lines }) : t('snippets.chars', { n: count(item.text) });
  return (
    <div className="snip-item">
      <code className="snip-chip">{item.abbr}</code>
      <div className="snip-main">
        <div className="snip-preview">{preview(item.text)}</div>
        <div className="row-desc">{meta}</div>
        {item.shadowed_by ? <div className="row-desc snip-warn">{t('snippets.shadow', { abbr: item.shadowed_by })}</div> : null}
      </div>
      <span className="snip-tools">
        <IconButton size="sm" variant="glass" icon="pencil" label={`${t('snippets.edit')}: ${item.abbr}`} onClick={onEdit} />
        <IconButton
          size="sm"
          variant="glass"
          icon="trash"
          label={`${t('snippets.remove')}: ${item.abbr}`}
          onClick={() =>
            confirmDialog({ title: t('snippets.remove.title'), body: t('snippets.remove.body', { abbr: item.abbr }), confirm: t('snippets.remove') }, () =>
              send({ t: 'set', path: 'snippets.remove', value: item.abbr }),
            )
          }
        />
      </span>
    </div>
  );
}

/** Snippets: type an abbreviation in any app and it is replaced by a longer text. */
export default function Snippets() {
  const t = useT();
  const s = useSection<SnippetsState>('snippets');
  const on = s.enabled;
  const [editing, setEditing] = useState<string | null>(null);
  const [fields, setFields] = useState({ abbr: '', text: '' });
  const abbrField = useRef<HTMLInputElement>(null);
  const edit = (item: Item) => {
    setEditing(item.abbr);
    setFields({ abbr: item.abbr, text: item.text });
    abbrField.current?.focus();
    abbrField.current?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  };
  return (
    <>
      {on && s.other ? (
        <Card>
          <Row title={t('snippets.other.title', { name: s.other })} desc={t('snippets.other.desc', { name: s.other })} />
        </Card>
      ) : null}
      <Card>
        <Row title={t('snippets.access.title')} desc={t('snippets.access.desc')} when={on}>
          <Pill good={s.accessibility}>{t(s.accessibility ? 'access.granted' : 'access.missing')}</Pill>
          <ActionIcon name="open_accessibility" icon="open" label={t('snippets.access.open')} />
        </Row>
      </Card>

      <SectionTitle>{t(editing === null ? 'sec.snippets.edit' : 'sec.snippets.editing')}</SectionTitle>
      <Card>
        <Row title={null} block when={on}>
          <Form s={s} on={on} editing={editing} fields={fields} setEditing={setEditing} setFields={setFields} abbrField={abbrField} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.snippets.list')}</SectionTitle>
      <Card>
        <Row title={null} block when={on}>
          <div className="snip-list">
            {s.items.length ? s.items.map((item) => <ItemRow key={item.abbr} item={item} onEdit={() => edit(item)} />) : <div className="row-desc">{t('snippets.none')}</div>}
          </div>
        </Row>
      </Card>

      <SectionTitle>{t('sec.snippets.how')}</SectionTitle>
      <Card>
        <InfoRow title={t('snippets.how.type.title')} desc={t('snippets.how.type.desc')} />
        <InfoRow title={t('snippets.how.paste.title')} desc={t('snippets.how.paste.desc')} />
        <InfoRow title={t('snippets.how.telex.title')} desc={t('snippets.how.telex.desc')} />
        <InfoRow title={t('snippets.how.stop.title')} desc={t('snippets.how.stop.desc')} />
      </Card>
    </>
  );
}
