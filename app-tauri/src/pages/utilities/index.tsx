import { Button, SelectRow, SliderRow, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import type { TFunction } from '@/stores/hooks';

interface UtilitiesState {
  enabled: boolean;
  awake: boolean;
  awake_left: number | null;
  mic_muted: boolean | null | undefined;
  snoozed: boolean;
  snooze_left: number | null;
  locked: boolean;
  accessibility: boolean;
  dark: boolean;
  desktop_icons: boolean;
}

/** A time left, as the page says it: seconds, minutes, or hours and minutes. */
function left(t: TFunction, secs: number): string {
  const unit = (key: string) => t(`util.unit.${key}`);
  if (secs < 60) return `${secs} ${unit('s')}`;
  if (secs < 3600) return `${Math.ceil(secs / 60)} ${unit('min')}`;
  return `${Math.floor(secs / 3600)} ${unit('h')} ${String(Math.ceil((secs % 3600) / 60)).padStart(2, '0')} ${unit('min')}`;
}

/** What a utility is doing now, and the button that starts or stops it. */
function StatusRow({ title, desc, state, action, label, disabled, when }: { title: string; desc: string; state: string; action: string; label: string; disabled?: boolean; when: boolean }) {
  return (
    <Row title={title} desc={desc} when={when}>
      <span className="util-state">{state}</span>
      <Button small disabled={disabled} onClick={() => send({ t: 'action', name: action })}>
        {label}
      </Button>
    </Row>
  );
}

/** The shortcut of a utility; they all share the words of the first one. */
function KeyRow({ target, label, when }: { target: string; label: string; when: boolean }) {
  const t = useT();
  return (
    <Row title={t('util.awake.shortcut')} desc={t('util.awake.shortcut.desc')} when={when}>
      <HotkeyButton target={target} allowClear label={label} />
    </Row>
  );
}

/** Utilities: keep the Mac awake, mute the microphone for every app, lock the keyboard to clean it, snooze the notch,
 *  switch the dark appearance, hide the desktop icons, and copy the link of the tab in front of a browser. */
export default function Utilities() {
  const t = useT();
  const u = useSection<UtilitiesState>('utilities');
  const on = u.enabled;
  const unit = (key: string) => t(`util.unit.${key}`);
  const hours = (m: number) => (m < 60 ? `${m} ${unit('min')}` : `${m / 60} ${unit('h')}`);
  const micKnown = u.mic_muted !== null && u.mic_muted !== undefined;
  return (
    <>
      <SectionTitle>{t('sec.util.awake')}</SectionTitle>
      <Card>
        <StatusRow
          title={t('util.awake.now.title')}
          desc={t('util.awake.now.desc')}
          state={!u.awake ? t('util.awake.off') : u.awake_left != null ? t('util.awake.on.left', { left: left(t, u.awake_left) }) : t('util.awake.on')}
          action="utilities_awake"
          label={u.awake ? t('util.awake.stop') : t('util.awake.start')}
          when={on}
        />
        <SelectRow
          path="utilities.awake_minutes"
          title={t('util.awake.length.title')}
          desc={t('util.awake.length.desc')}
          when={on}
          options={[15, 30, 60, 120, 240, 0].map((m) => ({ value: m, label: m === 0 ? t('util.length.until') : hours(m) }))}
        />
        <ToggleRow path="utilities.awake_display" title={t('util.awake.display.title')} desc={t('util.awake.display.desc')} when={on} />
        <Row title={t('util.awake.shortcut')} desc={t('util.awake.shortcut.desc')} when={on}>
          <HotkeyButton target="utilities:awake" allowClear label={t('util.awake.shortcut')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.util.mic')}</SectionTitle>
      <Card>
        <StatusRow
          title={t('util.mic.now.title')}
          desc={t('util.mic.now.desc')}
          state={!micKnown ? t('util.mic.unknown') : u.mic_muted ? t('util.mic.muted') : t('util.mic.live')}
          action="utilities_mic"
          label={u.mic_muted ? t('util.mic.unmute') : t('util.mic.mute')}
          disabled={!micKnown}
          when={on}
        />
        <KeyRow target="utilities:mic" label={t('util.mic.now.title')} when={on} />
      </Card>

      <SectionTitle>{t('sec.util.snooze')}</SectionTitle>
      <Card>
        <StatusRow
          title={t('util.snooze.now.title')}
          desc={t('util.snooze.now.desc')}
          state={!u.snoozed ? t('util.snooze.off') : u.snooze_left != null ? t('util.snooze.on.left', { left: left(t, u.snooze_left) }) : t('util.snooze.on')}
          action="utilities_snooze"
          label={u.snoozed ? t('util.snooze.stop') : t('util.snooze.start')}
          when={on}
        />
        <SelectRow
          path="utilities.snooze_minutes"
          title={t('util.snooze.length.title')}
          desc={t('util.snooze.length.desc')}
          when={on}
          options={[5, 15, 30, 60, 120, 0].map((m) => ({ value: m, label: m === 0 ? t('util.snooze.until') : hours(m) }))}
        />
        <Row title={t('util.snooze.shortcut')} desc={t('util.snooze.shortcut.desc')} when={on}>
          <HotkeyButton target="utilities:snooze" allowClear label={t('util.snooze.now.title')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.util.lock')}</SectionTitle>
      <Card>
        <StatusRow
          title={t('util.lock.now.title')}
          desc={t('util.lock.now.desc')}
          state={u.locked ? t('util.lock.locked') : t('util.lock.free')}
          action="utilities_lock"
          label={u.locked ? t('util.lock.stop') : t('util.lock.start')}
          when={on}
        />
        <SliderRow path="utilities.lock_seconds" title={t('util.lock.seconds.title')} desc={t('util.lock.seconds.desc')} min={15} max={300} step={5} format={(v) => left(t, v)} />
        <Row title={t('util.lock.access.title')} desc={t('util.lock.access.desc')} when={on}>
          <Pill good={u.accessibility}>{t(u.accessibility ? 'access.granted' : 'access.missing')}</Pill>
          <ActionIcon name="open_accessibility" icon="open" label={t('util.lock.access.open')} />
        </Row>
        <KeyRow target="utilities:lock" label={t('util.lock.now.title')} when={on} />
      </Card>

      <SectionTitle>{t('sec.util.look')}</SectionTitle>
      <Card>
        <StatusRow title={t('util.dark.title')} desc={t('util.dark.desc')} state={u.dark ? t('util.dark.on') : t('util.dark.off')} action="utilities_dark" label={t('util.dark.switch')} when={on} />
        <KeyRow target="utilities:dark" label={t('util.dark.title')} when={on} />
        <StatusRow
          title={t('util.desktop.title')}
          desc={t('util.desktop.desc')}
          state={u.desktop_icons ? t('util.desktop.shown') : t('util.desktop.hidden')}
          action="utilities_desktop"
          label={u.desktop_icons ? t('util.desktop.hide') : t('util.desktop.show')}
          when={on}
        />
        <KeyRow target="utilities:desktop" label={t('util.desktop.title')} when={on} />
        <Row title={t('util.automation.title')} desc={t('util.automation.desc')} when={on}>
          <ActionIcon name="utilities_automation" icon="open" label={t('util.lock.access.open')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.util.link')}</SectionTitle>
      <Card>
        <Row title={t('util.link.title')} desc={t('util.link.desc')} when={on}>
          <HotkeyButton target="utilities:link" allowClear label={t('util.link.title')} />
        </Row>
        <SelectRow
          path="utilities.link_format"
          title={t('util.link.format.title')}
          desc={t('util.link.format.desc')}
          when={on}
          options={['markdown', 'url', 'title_url'].map((f) => ({ value: f, label: t(`util.link.${f}`) }))}
        />
      </Card>
    </>
  );
}
