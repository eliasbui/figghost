import { Select, SelectRow, ToggleRow } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { change } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';

// One choice per figure: off, or how it is drawn. Off is its switch and the rest its kind, so a figure that was on before
// kinds existed stays on.
const KINDS: Record<string, string[]> = { processor: ['text', 'stacked', 'graph', 'cores'], network: ['text', 'stacked', 'graph'] };
const FIGURES = ['processor', 'memory', 'network', 'gpu', 'heat', 'disk'];

/** System: a tab of the notch with the processor, the memory, the network, the disk, the heat and the fans. */
export default function System() {
  const t = useT();
  const s = useSection<Record<string, any>>('system');
  const on = s.enabled as boolean;

  const figure = (id: string) => {
    const flag = `bar_${id}`;
    const kind = `bar_${id}_kind`;
    const title = t(`system.bar.${id}.title`);
    const label = (k: string) => t(`system.kind.${id === 'network' && k === 'stacked' ? 'rates' : k}`);
    return (
      <Row key={id} title={title} desc={t(`system.bar.${id}.desc`)} when={on}>
        <Select
          label={title}
          value={s[flag] ? s[kind] : 'off'}
          options={['off', ...(KINDS[id] ?? ['text', 'stacked', 'graph'])].map((k) => ({ value: k, label: label(k) }))}
          onChange={(v) => {
            if (v === 'off') {
              change(`system.${flag}`, false);
              return;
            }
            change(`system.${kind}`, v);
            change(`system.${flag}`, true);
          }}
        />
      </Row>
    );
  };

  return (
    <>
      <SectionTitle>{t('sec.system.shows')}</SectionTitle>
      <Card>
        <ToggleRow path="system.processor" title={t('system.processor.title')} desc={t('system.processor.desc')} when={on} />
        <ToggleRow path="system.memory" title={t('system.memory.title')} desc={t('system.memory.desc')} when={on} />
        <ToggleRow path="system.network" title={t('system.network.title')} desc={t('system.network.desc')} when={on} />
        <ToggleRow path="system.details" title={t('system.details.title')} desc={t('system.details.desc')} when={on} />
      </Card>

      <SectionTitle>{t('sec.system.bar')}</SectionTitle>
      <Card>
        {FIGURES.map(figure)}
        <Row title={t('system.bar.note.title')} desc={t('system.bar.note')} />
      </Card>
      <Card>
        <ToggleRow path="system.bar_combined" title={t('system.bar.combined.title')} desc={t('system.bar.combined.desc')} when={on} />
        <ToggleRow path="system.bar_color" title={t('system.bar.color.title')} desc={t('system.bar.color.desc')} when={on} />
      </Card>

      <SectionTitle>{t('sec.system.how')}</SectionTitle>
      <Card>
        <SelectRow
          path="system.seconds"
          title={t('system.seconds.title')}
          desc={t('system.seconds.desc')}
          when={on}
          options={[1, 2, 5].map((n) => ({ value: n, label: n === 1 ? t('system.every.one') : t('system.every.many', { n }) }))}
        />
      </Card>
    </>
  );
}
