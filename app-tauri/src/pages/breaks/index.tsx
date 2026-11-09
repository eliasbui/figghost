import { Button, SelectRow, ToggleRow } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';

interface BreaksState {
  enabled: boolean;
  breaking: boolean;
  until: number | null;
}

/** Breaks: a short break for the eyes after a while at the Mac, with reminders to blink and to sit up. */
export default function Breaks() {
  const t = useT();
  const b = useSection<BreaksState>('breaks');
  const on = b.enabled;
  const said = (secs: number) => (secs < 60 ? `${secs} ${t('breaks.s')}` : `${Math.ceil(secs / 60)} ${t('breaks.min')}`);
  const state = !b.enabled ? t('breaks.next.off') : b.breaking ? t('breaks.next.now') : b.until != null ? t('breaks.next.in', { left: said(b.until) }) : '';
  const nudge = [0, 10, 20, 30].map((m) => ({ value: m, label: m === 0 ? t('breaks.never') : t('breaks.every', { n: m }) }));
  return (
    <>
      <Card>
        <Row title={t('breaks.next.title')} when={on}>
          <span className="util-state">{state}</span>
          <Button small onClick={() => send({ t: 'action', name: 'breaks_now' })}>
            {b.breaking ? t('breaks.end') : t('breaks.now')}
          </Button>
        </Row>
      </Card>

      <SectionTitle>{t('sec.breaks.rhythm')}</SectionTitle>
      <Card>
        <SelectRow
          path="breaks.work_minutes"
          title={t('breaks.work.title')}
          desc={t('breaks.work.desc')}
          when={on}
          options={[20, 30, 45, 60].map((m) => ({ value: m, label: `${m} ${t('breaks.min')}` }))}
        />
        <SelectRow path="breaks.break_seconds" title={t('breaks.length.title')} desc={t('breaks.length.desc')} when={on} options={[20, 60, 300].map((s) => ({ value: s, label: said(s) }))} />
        <ToggleRow path="breaks.cover" title={t('breaks.cover.title')} desc={t('breaks.cover.desc')} when={on} />
        <ToggleRow path="breaks.sound" title={t('breaks.sound.title')} desc={t('breaks.sound.desc')} when={on} />
        <Row title={t('breaks.shortcut.title')} desc={t('breaks.shortcut.desc')} when={on}>
          <HotkeyButton target="breaks:now" allowClear label={t('breaks.now')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.breaks.nudges')}</SectionTitle>
      <Card>
        <SelectRow path="breaks.blink_minutes" title={t('breaks.blink.title')} desc={t('breaks.blink.desc')} when={on} options={nudge} />
        <SelectRow path="breaks.posture_minutes" title={t('breaks.posture.title')} desc={t('breaks.posture.desc')} when={on} options={nudge} />
      </Card>

      <SectionTitle>{t('sec.breaks.hold')}</SectionTitle>
      <Card>
        <ToggleRow path="breaks.hold_meetings" title={t('breaks.meetings.title')} desc={t('breaks.meetings.desc')} when={on} />
        <ToggleRow path="breaks.hold_full_screen" title={t('breaks.fullscreen.title')} desc={t('breaks.fullscreen.desc')} when={on} />
        <Row title={t('breaks.note.title')} desc={t('breaks.note')} />
      </Card>
    </>
  );
}
