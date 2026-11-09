import { useEffect, useRef, useState } from 'react';
import { Button, IconButton } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './commands.css';

interface Item {
  name: string;
  command: string;
}
interface CommandsState {
  enabled: boolean;
  items: Item[];
  max: number;
}

// A command with no name, no line or a control character in either is not good enough to be sent.
const good = (i: Item) => !!i.name.trim() && !!i.command.trim() && !/[\u0000-\u001f]/.test(i.name + i.command);

/** Commands: a tab of the notch that runs the commands you saved and shows what they print. */
export default function Commands() {
  const t = useT();
  const commands = useSection<CommandsState>('commands');
  const on = commands.enabled;
  // The list as typed: it goes to the app only when every command in it is good, so that nothing is lost half way.
  const [draft, setDraft] = useState<Item[]>(() => commands.items.map((i) => ({ ...i })));
  const [bad, setBad] = useState(false);
  const editing = useRef(false);

  const save = (list: Item[]) => {
    const ok = list.every(good);
    setBad(!ok);
    if (ok) send({ t: 'set', path: 'commands.items', value: list.map((i) => ({ name: i.name.trim(), command: i.command.trim() })) });
  };
  const update = (k: number, field: keyof Item, value: string) => {
    editing.current = true;
    setDraft((list) => list.map((item, i) => (i === k ? { ...item, [field]: value } : item)));
  };

  // What the app has is the truth, unless the person is typing.
  useEffect(() => {
    if (editing.current) return;
    setDraft((list) => (list.every(good) && JSON.stringify(list) !== JSON.stringify(commands.items) ? commands.items.map((i) => ({ ...i })) : list));
  }, [commands.items]);

  return (
    <>
      <SectionTitle info={t('commands.items.desc')}>{t('sec.commands.items')}</SectionTitle>
      <Card>
        <div className="row block">
          <div className="commands-list">
            {draft.map((item, k) => (
              <div className="commands-item" key={k}>
                <input
                  type="text"
                  className="text-field commands-name"
                  placeholder={t('commands.name')}
                  aria-label={t('commands.name')}
                  maxLength={40}
                  spellCheck={false}
                  value={item.name}
                  onChange={(e) => update(k, 'name', e.target.value)}
                  onBlur={() => {
                    editing.current = false;
                    save(draft);
                  }}
                />
                <input
                  type="text"
                  className="text-field commands-line"
                  placeholder={t('commands.line')}
                  aria-label={t('commands.line')}
                  maxLength={500}
                  spellCheck={false}
                  value={item.command}
                  onChange={(e) => update(k, 'command', e.target.value)}
                  onBlur={() => {
                    editing.current = false;
                    save(draft);
                  }}
                />
                <Button
                  small
                  icon="xmark"
                  aria-label={t('commands.remove', { name: item.name || String(k + 1) })}
                  onClick={() => {
                    const next = draft.filter((_, i) => i !== k);
                    setDraft(next);
                    save(next);
                  }}
                />
              </div>
            ))}
          </div>
          <div className="row-desc commands-note" role="status" style={{ display: bad ? '' : 'none' }}>
            {bad ? t('commands.bad') : ''}
          </div>
        </div>
        <Row title={t('commands.add')} when={on}>
          <IconButton icon="plus" variant="glass" label={t('commands.add')} disabled={!on || draft.length >= commands.max} onClick={() => draft.length < commands.max && setDraft([...draft, { name: '', command: '' }])} />
        </Row>
      </Card>
    </>
  );
}
