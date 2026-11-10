import { useEffect, useRef, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import { cn } from '@/lib/cn';
import { post } from './bridge';
import { ago, characters, newNoteId, noteTitle } from './logic';
import type { Note } from './logic';
import { Btn, Card, Grow, Header, Hint, Row } from './ui';

interface Props {
  t: (key: string, vars?: Record<string, string | number>) => string;
  notes: Note[];
  setNotes: Dispatch<SetStateAction<Note[] | null>>;
}

const SAVE_AFTER_MS = 600;

export function NotesPage({ t, notes, setNotes }: Props) {
  const [current, setCurrent] = useState<number | null>(null);
  const note = notes.find((n) => n.id === current) ?? notes[0];
  // The save runs a moment after the last key, from a timer, so it reads the latest of everything through refs.
  const latest = useRef({ notes, id: note?.id });
  latest.current = { notes, id: note?.id };
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const editor = useRef<HTMLTextAreaElement>(null);
  const focusNext = useRef(false);

  const save = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    const { notes: all, id } = latest.current;
    const open = all.find((n) => n.id === id);
    if (open) post({ t: 'note_save', id: open.id, text: open.text });
  };
  useEffect(() => () => {
    if (timer.current) save();
  }, []);
  useEffect(() => {
    if (focusNext.current) {
      focusNext.current = false;
      editor.current?.focus();
    }
  });

  const add = () => {
    save();
    const blank = notes.find((n) => !n.text.trim());
    const id = blank ? blank.id : newNoteId(notes, Date.now());
    if (!blank) setNotes([{ id, text: '', updated: Date.now() }, ...notes]);
    setCurrent(id);
    focusNext.current = true;
  };

  return (
    <>
      <Header title={t('page.notes')} sub={t('notes.sub')} />
      <p className="mb-3">
        <Btn variant="prominent" onClick={add}>
          {t('notes.new')}
        </Btn>
      </p>
      <div className="flex h-[calc(100vh-170px)] min-h-[300px] gap-3.5">
        <Card role="listbox" className="w-[230px] flex-none overflow-auto">
          {notes.length ? (
            notes.map((n) => (
              <Row
                key={n.id}
                role="option"
                aria-selected={n.id === note?.id}
                onClick={() => {
                  save();
                  setCurrent(n.id);
                }}
                className={cn('cursor-default flex-col items-start gap-0', n.id === note?.id && 'bg-accent-soft')}
              >
                <div className="w-full truncate font-semibold">{noteTitle(n.text, t('notes.untitled'))}</div>
                <Hint>{t(ago(n.updated, Date.now()).key, { n: ago(n.updated, Date.now()).n })}</Hint>
              </Row>
            ))
          ) : (
            <div className="p-[30px] text-center text-fg-2">{t('notes.empty')}</div>
          )}
        </Card>
        {note ? (
          <div className="flex min-w-0 flex-1 flex-col gap-2">
            <textarea
              ref={editor}
              key={note.id}
              aria-label={noteTitle(note.text, t('notes.untitled'))}
              maxLength={50000}
              defaultValue={note.text}
              className="min-h-0 flex-1 resize-none rounded-[var(--r-card)] bg-card p-3 text-sm leading-[1.55] shadow-[inset_0_0_0_0.5px_var(--card-edge)] select-text focus:shadow-[inset_0_0_0_0.5px_var(--accent),0_0_0_3px_var(--accent-ring)]"
              onChange={(event) => {
                const text = event.target.value;
                setNotes((all) => (all ?? []).map((n) => (n.id === note.id ? { ...n, text, updated: Date.now() } : n)));
                if (timer.current) clearTimeout(timer.current);
                timer.current = setTimeout(save, SAVE_AFTER_MS);
              }}
              onBlur={() => {
                if (timer.current) save();
              }}
            />
            <Card>
              <Row>
                <Grow>
                  <Hint>{t('notes.chars', { n: characters(note.text) })}</Hint>
                </Grow>
                <Btn
                  variant="danger"
                  onClick={() => {
                    if (!confirm(t('notes.delete.ask'))) return;
                    if (timer.current) clearTimeout(timer.current);
                    timer.current = null;
                    post({ t: 'note_delete', id: note.id });
                    setCurrent(null);
                  }}
                >
                  {t('notes.delete')}
                </Btn>
              </Row>
            </Card>
          </div>
        ) : (
          <div className="flex-1" />
        )}
      </div>
    </>
  );
}
