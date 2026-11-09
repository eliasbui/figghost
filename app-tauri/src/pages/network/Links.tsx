import { IconButton, Select } from '@/components/macos/Controls';
import { InfoTip } from '@/components/macos/Overlay';
import { send } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { mbps } from './helpers';
import type { Link, NetworkState } from './types';

const act = (name: string) => send({ t: 'action', name });

/** One connection: its address and how well it answers, its Wi-Fi and what is wrong with the web through it, the choice of
 *  whether it is metered, and a test of its speed. */
function LinkRow({ link: l, network }: { link: Link; network: NetworkState }) {
  const t = useT();
  const m = network.metered;
  const q = l.quality && l.quality !== 'unknown' ? (l.quality === 'down' ? t('network.q.down') : t(`network.q.${l.quality}`, { ms: l.rtt ?? 0, loss: Math.round((l.loss || 0) * 100) })) : '';
  const w = l.wifi;
  // What is wrong with the web through the connection, found by asking it and not by the answer of a handshake.
  const test = l.speed_test;
  const web = l.web || (test && test.web ? { http: test.web, https: test.https ?? '' } : null);
  const problem = !web ? '' : web.http === 'none' ? 'none' : web.http === 'login' ? 'login' : web.https === 'tls' ? 'tls' : web.https === 'fail' ? 'fail' : '';
  let result = '';
  if (test && test.state === 'done') {
    const text = test.up == null ? t('network.speed.done.noup', { down: mbps(test.down), rtt: test.rtt ?? 0 }) : t('network.speed.done', { down: mbps(test.down), up: mbps(test.up), rtt: test.rtt ?? 0 });
    result = test.plain ? `${text} ${t('network.speed.plain')}` : text;
  } else if (test && test.state === 'failed') {
    result = t(`network.speed.failed.${test.reason}`);
  }
  const mark = (m.marked ?? []).includes(l.name) ? 'metered' : (m.unmarked ?? []).includes(l.name) ? 'not' : 'auto';
  const running = !!(test && test.state === 'running');
  return (
    <div className="row">
      <div className="row-text">
        <div className="row-title">
          <span className="row-name">{`${l.label} (${l.name})`}</span>
          {w ? <InfoTip>{`${t(`network.wifi.grade.${w.grade}`)}: ${t('network.wifi.line', { standard: w.standard || '802.11', channel: w.channel, band: w.band, width: w.width, rssi: w.rssi, noise: w.noise, rate: w.rate })}`}</InfoTip> : null}
          {problem ? (
            <InfoTip bad text={t('ui.problem')}>
              {t(`network.web.${problem}`)}
            </InfoTip>
          ) : null}
        </div>
        <div className="row-sub">{[l.ip, q].filter(Boolean).join(' - ')}</div>
        {result && !running ? (
          <div className="row-sub network-speed-result" data-state={test ? test.state : ''}>
            {result}
          </div>
        ) : null}
      </div>
      <div className="row-control network-link-control">
        {m.enabled ? (
          <Select
            small
            label={t('network.metered.mark')}
            value={mark}
            options={['auto', 'metered', 'not'].map((v) => ({ value: v, label: t(`network.metered.mark.${v}`) }))}
            onChange={(v) => send({ t: 'set', path: 'network.metered.mark', value: { name: l.name, state: String(v) } })}
          />
        ) : null}
        <IconButton
          variant="glass"
          icon={test ? 'refresh' : 'play'}
          loading={running}
          label={running ? t('network.speed.running') : test ? t('network.speed.again') : t('network.speed.test')}
          onClick={() => act(`network_speed_test:${l.name}`)}
        />
      </div>
    </div>
  );
}

/** Each connection with how well it answers, the radio of the Wi-Fi one, and a test of its speed. */
export function Links({ network }: { network: NetworkState }) {
  const t = useT();
  const links = (network.links ?? []).filter((l) => l.name);
  return (
    <div className="network-linklist">
      {links.length ? links.map((l) => <LinkRow key={l.name} link={l} network={network} />) : <div className="row-desc network-empty">{t('network.links.none')}</div>}
    </div>
  );
}
