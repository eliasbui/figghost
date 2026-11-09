import { useEffect, useRef, useState } from 'react';
import { Button, IconButton, SegRow, Select, SelectRow, ToggleRow } from '@/components/macos/Controls';
import { confirmDialog } from '@/components/macos/Dialogs';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './launcher.css';

interface Group {
  name: string;
  items: Array<{ kind: string; name: string }>;
  full: boolean;
}
interface LauncherState {
  enabled: boolean;
  dock: boolean;
  dock_display: string;
  groups: Group[];
  groups_full: boolean;
  shortcuts: string[];
  listing: boolean;
  problem: string;
  screens: string[];
}

const act = (name: string) => send({ t: 'action', name });

/** A line of text that is sent when Return is pressed in it (and not while an input method is composing). */
function Entry({ value, onChange, onEnter, label, placeholder, max, className }: { value: string; onChange: (v: string) => void; onEnter: () => void; label: string; placeholder: string; max: number; className?: string }) {
  return (
    <input
      type="text"
      className={`text-field${className ? ` ${className}` : ''}`}
      aria-label={label}
      placeholder={placeholder}
      maxLength={max}
      spellCheck={false}
      autoComplete="off"
      autoCapitalize="off"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' && !e.nativeEvent.isComposing) onEnter();
      }}
    />
  );
}

/** Launcher: the apps, folders, files, web addresses and Shortcuts you pinned, in groups, in a tab of the notch and in a
 *  dock at an edge of a screen. */
export default function Launcher() {
  const t = useT();
  const l = useSection<LauncherState>('launcher');
  const on = l.enabled || l.dock;
  // The group this page edits; the tab and the dock each show their own.
  const [sel, setSel] = useState(0);
  const at = Math.max(0, Math.min(sel, l.groups.length - 1));
  const cur = l.groups[at];
  const label = (g: Group, i: number) => g.name || t('launcher.group.n', { n: i + 1 });

  const [name, setName] = useState(cur?.name ?? '');
  const naming = useRef(false);
  useEffect(() => {
    if (!naming.current) setName(cur?.name ?? '');
  }, [cur?.name, at]);
  const rename = () => act(`launcher_group_rename:${at}:${name.trim()}`);

  const [address, setAddress] = useState('');
  const pinUrl = () => {
    const trimmed = address.trim();
    if (!trimmed) return;
    act(`launcher_add_url:${at}:${trimmed}`);
    setAddress('');
  };
  const [shortcut, setShortcut] = useState(0);

  if (!cur) return null;
  const screens = [{ value: '', label: t('launcher.dock_display.main') }, ...l.screens.map((n) => ({ value: n, label: n }))];
  if (l.dock_display && !l.screens.includes(l.dock_display)) screens.push({ value: l.dock_display, label: t('launcher.dock_display.away', { name: l.dock_display }) });

  const tool = (verb: string, item: number, glyph: string, text: string, disabled: boolean, who: string) => (
    <IconButton size="sm" variant="glass" icon={glyph} disabled={disabled} label={`${text}: ${who}`} onClick={() => act(`${verb}:${at}:${item}`)} />
  );

  return (
    <>
      <SectionTitle>{t('sec.launcher.groups')}</SectionTitle>
      <Card>
        <Row title={t('launcher.group.title')} desc={t('launcher.group.desc')} when={on}>
          <div className="launcher-add">
            <Select label={t('launcher.group.pick')} value={at} options={l.groups.map((g, i) => ({ value: i, label: label(g, i) }))} onChange={(v) => setSel(Number(v))} />
            <IconButton
              variant="glass"
              icon="chevron.up"
              disabled={at === 0}
              label={t('launcher.group.up')}
              onClick={() => {
                act(`launcher_group_up:${at}`);
                setSel(at - 1);
              }}
            />
            <IconButton
              variant="glass"
              icon="chevron.down"
              disabled={at === l.groups.length - 1}
              label={t('launcher.group.down')}
              onClick={() => {
                act(`launcher_group_down:${at}`);
                setSel(at + 1);
              }}
            />
            <IconButton
              variant="glass"
              icon="plus"
              disabled={l.groups_full}
              label={t('launcher.group.add')}
              onClick={() => {
                setSel(l.groups.length);
                act('launcher_group_add');
              }}
            />
          </div>
        </Row>
        <Row title={t('launcher.group.name')} when={on}>
          <div className="launcher-add nowrap">
            <input
              type="text"
              className="text-field"
              aria-label={t('launcher.group.name')}
              placeholder={t('launcher.group.ph')}
              maxLength={24}
              spellCheck={false}
              autoComplete="off"
              value={name}
              onFocus={() => {
                naming.current = true;
              }}
              onBlur={() => {
                naming.current = false;
              }}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.nativeEvent.isComposing) rename();
              }}
            />
            <IconButton variant="glass" icon="check" label={t('launcher.group.rename')} onClick={rename} />
            <IconButton
              variant="danger"
              icon="trash"
              disabled={l.groups.length < 2}
              label={t('launcher.group.remove')}
              onClick={() =>
                confirmDialog({ title: t('launcher.group.confirm', { name: label(cur, at) }), body: t('launcher.group.confirm.body'), confirm: t('launcher.group.remove') }, () => {
                  act(`launcher_group_remove:${at}`);
                  setSel(Math.max(0, at - 1));
                })
              }
            />
          </div>
        </Row>
      </Card>

      <SectionTitle>{t('sec.launcher.pinned', { name: label(cur, at) })}</SectionTitle>
      <Card>
        <Row title={null} block when={on}>
          <div className="launcher-items">
            {cur.items.length ? (
              cur.items.map((item, i) => (
                <div className="launcher-item" key={`${i}:${item.name}`}>
                  <span className="launcher-kind">{t(`launcher.kind.${item.kind}`)}</span>
                  <span className="launcher-name">{item.name}</span>
                  <span className="launcher-tools">
                    {tool('launcher_up', i, 'chevron.up', t('launcher.up'), i === 0, item.name)}
                    {tool('launcher_down', i, 'chevron.down', t('launcher.down'), i === cur.items.length - 1, item.name)}
                    {tool('launcher_remove', i, 'xmark', t('launcher.remove'), false, item.name)}
                  </span>
                </div>
              ))
            ) : (
              <div className="row-desc">{t('launcher.none')}</div>
            )}
          </div>
        </Row>
      </Card>

      <SectionTitle>{t('sec.launcher.add')}</SectionTitle>
      <Card>
        <Row title={t('launcher.add.title')} desc={t('launcher.add.desc')} when={on}>
          <div className="launcher-add">
            <IconButton variant="glass" icon="app.dashed" disabled={cur.full} label={t('launcher.add.app')} onClick={() => act(`launcher_add_app:${at}`)} />
            <IconButton variant="glass" icon="folder" disabled={cur.full} label={t('launcher.add.folder')} onClick={() => act(`launcher_add_folder:${at}`)} />
            <IconButton variant="glass" icon="doc" disabled={cur.full} label={t('launcher.add.file')} onClick={() => act(`launcher_add_file:${at}`)} />
          </div>
        </Row>
        <Row title={t('launcher.url.title')} desc={t('launcher.url.desc')} when={on}>
          <div className="launcher-add nowrap">
            <Entry className="launcher-url" value={address} onChange={setAddress} onEnter={pinUrl} label={t('launcher.url.label')} placeholder={t('launcher.url.ph')} max={2000} />
            <IconButton variant="glass" icon="pin" disabled={cur.full} label={t('launcher.url.add')} onClick={pinUrl} />
          </div>
        </Row>
        <Row title={t('launcher.shortcuts.title')} desc={t('launcher.shortcuts.desc')} when={on}>
          <div className="launcher-add">
            <Button disabled={l.listing} onClick={() => act('launcher_shortcuts')}>
              {t('launcher.shortcuts.load')}
            </Button>
            {l.shortcuts.length ? (
              <>
                <Select label={t('launcher.shortcuts.pick')} value={Math.min(shortcut, l.shortcuts.length - 1)} options={l.shortcuts.map((n, i) => ({ value: i, label: n }))} onChange={(v) => setShortcut(Number(v))} />
                <IconButton variant="glass" icon="pin" disabled={cur.full} label={t('launcher.shortcuts.add')} onClick={() => act(`launcher_add_shortcut:${at}:${Math.min(shortcut, l.shortcuts.length - 1)}`)} />
              </>
            ) : null}
          </div>
        </Row>
        <Row title={null} block when={on}>
          <div className="row-desc launcher-status">
            {l.listing ? t('launcher.shortcuts.reading') : l.problem === 'none' ? t('launcher.shortcuts.none') : l.problem === 'slow' ? t('launcher.shortcuts.slow') : ''}
          </div>
        </Row>
      </Card>

      <SectionTitle>{t('sec.launcher.dock')}</SectionTitle>
      <Card>
        <ToggleRow path="launcher.dock" title={t('launcher.dock.title')} desc={t('launcher.dock.desc')} />
        <SegRow path="launcher.dock_edge" title={t('launcher.dock_edge.title')} desc={t('launcher.dock_edge.desc')} values={['left', 'right', 'bottom']} when={l.dock} />
        <SelectRow path="launcher.dock_display" title={t('launcher.dock_display.title')} desc={t('launcher.dock_display.desc')} when={l.dock} options={screens} />
        <SelectRow path="launcher.dock_group" title={t('launcher.dock_group.title')} desc={t('launcher.dock_group.desc')} when={l.dock} options={l.groups.map((g, i) => ({ value: i, label: label(g, i) }))} />
        <ToggleRow path="launcher.dock_autohide" title={t('launcher.dock_autohide.title')} desc={t('launcher.dock_autohide.desc')} when={l.dock} />
      </Card>
    </>
  );
}
