import { ListItem } from '@astryxdesign/core/List';
import { VStack } from '@astryxdesign/core/VStack';
import { SureButton } from '@/ui/controls';
import { HotkeyButton } from '@/ui/hotkey';
import { Name, Section } from '@/ui/layout';
import { SelectRow, ToggleRow } from '@/ui/rows';
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
    <VStack gap={6}>
      <Section title={t('sec.pomodoro.lengths')}>
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
      </Section>

      <Section title={t('sec.pomodoro.when')}>
        <ToggleRow path="pomodoro.auto_start" title={t('pomodoro.auto.title')} desc={t('pomodoro.auto.desc')} when={on} />
        <ToggleRow path="pomodoro.sound" title={t('pomodoro.sound.title')} desc={t('pomodoro.sound.desc')} when={on} />
        <ToggleRow path="pomodoro.open_on_end" title={t('pomodoro.open.title')} desc={t('pomodoro.open.desc')} when={on} />
        <ToggleRow path="pomodoro.closed_notch" title={t('pomodoro.closed.title')} desc={t('pomodoro.closed.desc')} when={on} />
        <ListItem
          label={<Name title={t('pomodoro.shortcut.title')} desc={t('pomodoro.shortcut.desc')} />}
          isDisabled={!on}
          endContent={<HotkeyButton target="pomodoro:toggle" allowClear label={t('pomodoro.shortcut.title')} />}
        />
      </Section>

      <Section title={t('sec.pomodoro.stats')}>
        <ListItem
          label={t('pomodoro.stats.title')}
          description={t('pomodoro.stats.desc', { today: p.today, streak: p.streak, days: p.days })}
          isDisabled={!on}
          endContent={<SureButton label={t('pomodoro.clear')} sure={t('pomodoro.clear.sure')} onConfirm={() => send({ t: 'action', name: 'pomodoro_clear' })} />}
        />
      </Section>
    </VStack>
  );
}
