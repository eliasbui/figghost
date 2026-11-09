import { ActionButton, ActionIcon } from '@/components/macos/Controls';
import { Icon } from '@/components/macos/Icon';
import { InfoTip } from '@/components/macos/Overlay';
import { useT } from '@/stores/hooks';
import { size, speed } from './helpers';
import type { NetworkState } from './types';

/** A full bar is 20 MB/s, on a scale of tens, so that a slow phone still moves its bar. */
const level = (b: number | undefined) => Math.min(100, Math.max(0, (Math.log10(1 + (Number(b) || 0) / 1e3) / Math.log10(2e4)) * 100));

function Gauge({ label, bytesPerSecond }: { label: string; bytesPerSecond: number | undefined }) {
  return (
    <div className="phone-gauge">
      <span className="phone-gauge-label">{label}</span>
      <span className="phone-gauge-value">{speed(bytesPerSecond)}</span>
      <span className="phone-gauge-bar">
        <span className="phone-gauge-fill" style={{ width: `${level(bytesPerSecond)}%` }} />
      </span>
    </div>
  );
}

/** Phone sharing: one card that says where it stands with a coloured light, and the one button that moves on. */
export function Phone({ network }: { network: NetworkState }) {
  const t = useT();
  const tt = network.tether ?? {};
  const tools = tt.gnirehtet && tt.adb;
  const phones = tt.phones ?? [];
  const model = phones.map((p) => p.model || p.serial).join(', ');
  const state = !tools ? 'missing' : tt.running ? (phones.length ? 'running' : 'lost') : tt.unauthorized ? 'unauthorized' : phones.length ? 'ready' : 'none';
  const test = tt.test ?? { state: 'idle' };
  const m = tt.meter ?? {};
  const testing = test.state === 'running';
  const verdict = test.state === 'done' ? test.verdict : test.state;
  const testText = testing
    ? t('network.tether.testing')
    : test.state === 'done'
      ? (test.phone ? `${test.phone}: ` : '') + t(`network.tether.test.${test.verdict}`, { ms: test.ms == null ? '-' : test.ms, loss: test.loss ?? 0 })
      : '';
  return (
    <div className="phone-card" data-state={state}>
      <div className="phone-badge">
        <Icon name="iphone.radiowaves.left.and.right" className="phone-glyph" />
        <span className="phone-light" aria-hidden="true" />
      </div>
      <div className="phone-text">
        <div className="phone-name">
          {t('network.tether.title')}
          {state === 'missing' ? null : <InfoTip>{t(`network.tether.${state}.hint`)}</InfoTip>}
        </div>
        <div className="phone-state" hidden={state === 'missing'}>
          {state === 'missing' ? '' : t(`network.tether.${state}`, { model })}
        </div>
        {state === 'missing' ? <div className="phone-hint">{t('network.tether.missing')}</div> : null}
        {/* The command is written only while it is needed: the card does not name the tool it runs otherwise. */}
        <code className="code phone-install" hidden={state !== 'missing'}>
          {state === 'missing' ? 'brew install gnirehtet android-platform-tools' : ''}
        </code>
      </div>
      <div className="phone-actions">
        {state === 'ready' ? <ActionButton name="network_tether_start" primary>{t('network.tether.start')}</ActionButton> : null}
        {state === 'ready' ? <ActionIcon name="network_tether_stop" icon="power" label={t('network.tether.vpnoff')} /> : null}
        {tt.running ? <ActionButton name="network_tether_stop" danger>{t('network.tether.stop')}</ActionButton> : null}
        {(state === 'ready' || state === 'running') && !testing ? <ActionIcon name="network_tether_test" icon="wifi" label={t('network.tether.test')} /> : null}
      </div>
      <div className="phone-meter" hidden={state !== 'running'}>
        <Gauge label={t('network.tether.down')} bytesPerSecond={m.down} />
        <Gauge label={t('network.tether.up')} bytesPerSecond={m.up} />
        <div className="phone-used">{`${t('network.tether.used')}: ${size((m.received || 0) + (m.sent || 0))}`}</div>
      </div>
      <div className="phone-test" data-verdict={verdict} hidden={!testText}>
        {testText}
      </div>
    </div>
  );
}
