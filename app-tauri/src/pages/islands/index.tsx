import { ActionIcon, SliderRow, Switch } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { useSection, useT } from '@/stores/hooks';
import './islands.css';

interface IslandsState {
  enabled: boolean;
  has_battery: boolean;
}

/** One notice: the switch that lets it come out of the notch, and a button that shows a sample of it. */
function Notice({ name, kind, setting, on }: { name: string; kind: string; setting: string; on: boolean }) {
  const t = useT();
  return (
    <Row title={t(`islands.${name}.title`)} desc={t(`islands.${name}.desc`)} when={on}>
      <ActionIcon name={`islands_sample:${kind}`} icon="play" label={t('islands.sample')} />
      <Switch path={`islands.${setting}`} label={t(`islands.${name}.title`)} />
    </Row>
  );
}

/** Live islands: small notices that come out of the closed notch when something happens on the Mac. */
export default function Islands() {
  const t = useT();
  const islands = useSection<IslandsState>('islands');
  const on = islands.enabled;
  return (
    <>
      <SectionTitle>{t('sec.islands.which')}</SectionTitle>
      <Card>
        <Notice name="caps" kind="caps" setting="caps_lock" on={on} />
        <Notice name="power" kind="plug" setting="power" on={on} />
        <Notice name="drives" kind="drive" setting="drives" on={on} />
        <Notice name="downloads" kind="download" setting="downloads" on={on} />
        <Notice name="camera" kind="camera" setting="camera" on={on} />
        <Notice name="microphone" kind="mic" setting="microphone" on={on} />
      </Card>

      <SectionTitle>{t('sec.islands.battery')}</SectionTitle>
      {islands.has_battery ? (
        <Card>
          <SliderRow path="islands.full_at" title={t('islands.full.title')} desc={t('islands.full.desc')} min={80} max={100} step={1} format={(v) => `${v}%`} when={on} />
          <SliderRow path="islands.low_at" title={t('islands.low.title')} desc={t('islands.low.desc')} min={5} max={30} step={1} format={(v) => `${v}%`} when={on} />
        </Card>
      ) : (
        <Card>
          <div className="islands-note">{t('islands.nobattery')}</div>
        </Card>
      )}

      <SectionTitle>{t('sec.islands.time')}</SectionTitle>
      <Card>
        <SliderRow path="islands.seconds" title={t('islands.seconds.title')} desc={t('islands.seconds.desc')} min={2} max={10} step={0.5} format={(v) => `${v.toFixed(1)} s`} when={on} />
      </Card>
    </>
  );
}
