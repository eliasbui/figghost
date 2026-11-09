import { SelectRow, ToggleRow } from '@/components/macos/Controls';
import { Card, SectionTitle } from '@/components/macos/Layout';
import { useSetting, useT } from '@/stores/hooks';

/** Day progress: a tab of the notch with how much of the day has gone and today's events laid along a line. */
export default function DayProgress() {
  const t = useT();
  const on = useSetting<boolean>('dayprogress.enabled');
  const hour = (h: number) => (h === 24 ? t('dayprogress.midnight') : t(h < 12 ? 'dayprogress.am' : 'dayprogress.pm', { h: h % 12 || 12, h24: h }));
  const from = (hours: number[]) => hours.map((h) => ({ value: h, label: hour(h) }));
  return (
    <>
      <SectionTitle>{t('sec.dayprogress.line')}</SectionTitle>
      <Card>
        <SelectRow path="dayprogress.wake" title={t('dayprogress.wake.title')} desc={t('dayprogress.wake.desc')} when={on} options={from([5, 6, 7, 8, 9, 10])} />
        <SelectRow path="dayprogress.bed" title={t('dayprogress.bed.title')} desc={t('dayprogress.bed.desc')} when={on} options={from([21, 22, 23, 24])} />
      </Card>

      <SectionTitle>{t('sec.dayprogress.reminders')}</SectionTitle>
      <Card>
        <ToggleRow path="dayprogress.reminders" title={t('dayprogress.reminders.title')} desc={t('dayprogress.reminders.desc')} when={on} />
      </Card>
    </>
  );
}
