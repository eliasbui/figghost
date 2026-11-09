import { SelectRow, SureButton, ToggleRow } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';

/** Pomodoro: a tab of the notch that alternates work sessions and breaks, with a ring, a countdown in the closed notch and
 *  a count of the days. */
export default function Pomodoro() {
  const t = useT();
  const p = useSection<{ enabled: boolean; today: number; streak: number; days: number }>('pomodoro');
  const on = p.enabled;
  const minutes = (list: number[]) => list.map((n) => ({ value: n, label: t('pomodoro.min', { n }) }));
  return (
    <>
      <SectionTitle>{t('sec.pomodoro.lengths')}</SectionTitle>
      <Card>
        <SelectRow path="pomodoro.work" title={t('pomodoro.work.title')} desc={t('pomodoro.work.desc')} when={on} options={minutes([15, 20, 25, 30, 45, 50, 60])} />
        <SelectRow path="pomodoro.short_break" title={t('pomodoro.short.title')} desc={t('pomodoro.short.desc')} when={on} options={minutes([3, 5, 10, 15])} />
        <SelectRow path="pomodoro.long_break" title={t('pomodoro.long.title')} desc={t('pomodoro.long.desc')} when={on} options={minutes([10, 15, 20, 30])} />
        <SelectRow
          path="pomodoro.rounds"
          title={t('pomodoro.rounds.title')}
          desc={t('pomodoro.rounds.desc')}
          when={on}
          options={[2, 3, 4, 5, 6].map((n) => ({ value: n, label: t('pomodoro.sessions', { n }) }))}
        />
      </Card>

      <SectionTitle>{t('sec.pomodoro.when')}</SectionTitle>
      <Card>
        <ToggleRow path="pomodoro.auto_start" title={t('pomodoro.auto.title')} desc={t('pomodoro.auto.desc')} when={on} />
        <ToggleRow path="pomodoro.sound" title={t('pomodoro.sound.title')} desc={t('pomodoro.sound.desc')} when={on} />
        <ToggleRow path="pomodoro.open_on_end" title={t('pomodoro.open.title')} desc={t('pomodoro.open.desc')} when={on} />
        <ToggleRow path="pomodoro.closed_notch" title={t('pomodoro.closed.title')} desc={t('pomodoro.closed.desc')} when={on} />
        <Row title={t('pomodoro.shortcut.title')} desc={t('pomodoro.shortcut.desc')} when={on}>
          <HotkeyButton target="pomodoro:toggle" allowClear label={t('pomodoro.shortcut.title')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.pomodoro.stats')}</SectionTitle>
      <Card>
        <Row title={t('pomodoro.stats.title')} below={t('pomodoro.stats.desc', { today: p.today, streak: p.streak, days: p.days })} when={on}>
          <SureButton label={t('pomodoro.clear')} sure={t('pomodoro.clear.sure')} onConfirm={() => send({ t: 'action', name: 'pomodoro_clear' })} />
        </Row>
      </Card>
    </>
  );
}
