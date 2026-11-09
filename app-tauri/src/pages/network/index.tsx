import { useEffect, useRef, useState } from 'react';
import { SelectRow, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { change } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import { useChipEditor } from './ChipEditor';
import { ExportControls, History, WipeButton } from './History';
import { Links } from './Links';
import { Phone } from './Phone';
import { linkName, size, speed } from './helpers';
import type { NetworkState } from './types';
import './network.css';

const DNS_CHOICES = ['cloudflare', 'quad9', 'adguard', 'google', 'nextdns', 'custom'];
const CAPS = [0, 500, 1000, 2000, 5000, 10000, 20000, 50000];
const sizeMb = (mb: number) => (mb >= 1000 ? `${mb / 1000} GB` : `${mb} MB`);

/** A text setting that is written when the field is left; the state of the app fills it in meanwhile, unless it is being typed in. */
function DnsField({ path, value, label, hint }: { path: string; value: string; label: string; hint: string }) {
  const [text, setText] = useState(value);
  const typing = useRef(false);
  useEffect(() => {
    if (!typing.current) setText(value || '');
  }, [value]);
  return (
    <Row title={label}>
      <input
        type="text"
        className="text-field network-dns-input"
        placeholder={hint}
        aria-label={label}
        spellCheck={false}
        autoComplete="off"
        value={text}
        onFocus={() => {
          typing.current = true;
        }}
        onChange={(e) => setText(e.target.value)}
        onBlur={() => {
          typing.current = false;
          change(path, text.trim());
        }}
      />
    </Row>
  );
}

/** Network: what each app moves and through which connection, and (beta) wires that choose the connection of an app, its speed
 *  and its place behind the apps wired before it. The diagram is drawn in a window of its own (Canvas.tsx). */
export default function Network() {
  const t = useT();
  const n = useSection<NetworkState>('network');
  const on = n.enabled;
  const m = n.metered;
  const meteredOn = on && m.enabled;
  const p = n.policy ?? {};

  const allow = useChipEditor({
    network: n,
    path: 'network.metered.allow',
    list: m.allow ?? [],
    none: t('network.metered.allow.none'),
    add: t('network.metered.allow.add'),
    remove: (app) => t('network.metered.allow.remove', { app }),
  });
  const ignore = useChipEditor({
    network: n,
    path: 'network.anomaly_ignore',
    list: n.anomaly_ignore ?? [],
    none: t('network.anomaly.ignore.none'),
    add: t('network.metered.allow.add'),
    remove: (app) => t('network.metered.allow.remove', { app }),
  });

  const cap = (m.cap_mb || 0) * 1e6;
  const meteredLines = [p.metered_now ? t('network.metered.now', { link: p.link || '' }) : t('network.metered.not')];
  if (cap) meteredLines.push(t('network.metered.cap.used', { used: size(p.cap_used || 0), cap: size(cap) }));
  if (p.cap_level === 'over') meteredLines.push(t('network.metered.cap.over'));

  const apps = n.apps ?? [];
  const dnsBlocked = (n.dns === 'nextdns' && !n.dns_nextdns) || (n.dns === 'custom' && !n.dns_custom);

  return (
    <>
      <Card>
        <Row title={t('network.open')} desc={t('network.open.desc')} when={on}>
          <ActionIcon name="network_canvas_open" icon="open" variant="prominent" label={t('network.open')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.network.summary')}</SectionTitle>
      <Card>
        <Row title={<div className="network-total">{`${t('network.total')}: ${speed(n.total_down)} / ${speed(n.total_up)}`}</div>} when={on} />
        <div className="row block network-table-row">
          <div className="network-table" role="table">
            {apps.length ? (
              <>
                <div className="network-tr head" role="row">
                  {['app', 'down', 'up', 'total', 'via'].map((k) => (
                    <span key={k} role="columnheader">
                      {t(`network.col.${k}`)}
                    </span>
                  ))}
                </div>
                {apps.slice(0, 12).map((a) => (
                  <div className="network-tr" role="row" key={a.name}>
                    <span className="network-app">{a.name}</span>
                    <span>{speed(a.down)}</span>
                    <span>{speed(a.up)}</span>
                    <span>{size(a.received + a.sent)}</span>
                    <span className="network-via">{(a.interfaces ?? []).map((i) => linkName(n, i.name)).join(', ') || '-'}</span>
                  </div>
                ))}
              </>
            ) : (
              <div className="row-desc network-empty">{t('network.apps.none')}</div>
            )}
          </div>
        </div>
      </Card>

      <SectionTitle>{t('sec.network.metered')}</SectionTitle>
      <Card>
        <ToggleRow path="network.metered.enabled" title={t('network.metered.title')} desc={t('network.metered.desc')} when={on} />
        <ToggleRow path="network.metered.auto" title={t('network.metered.auto.title')} desc={t('network.metered.auto.desc')} when={meteredOn} />
        <Row title={t('network.metered.allow.title')} desc={t('network.metered.allow.desc')} when={meteredOn}>
          {allow.controls}
        </Row>
        <div className={meteredOn ? 'row block' : 'row block disabled'}>{allow.chips}</div>
        <SelectRow
          path="network.metered.cap_mb"
          title={t('network.metered.cap.title')}
          desc={t('network.metered.cap.desc')}
          when={meteredOn}
          options={CAPS.map((v) => ({ value: v, label: v ? sizeMb(v) : t('network.metered.cap.none') }))}
        />
        <SelectRow
          path="network.metered.period"
          title={t('network.metered.period.title')}
          when={meteredOn && m.cap_mb > 0}
          options={['day', 'month'].map((v) => ({ value: v, label: t(`network.period.${v}`) }))}
        />
        <Row
          when={on}
          title={
            <div className="row-desc network-status" data-state={p.cap_level === 'over' ? 'failed' : p.metered_now ? 'on' : 'off'}>
              {meteredLines.join(' ')}
            </div>
          }
        />
      </Card>

      <SectionTitle>{t('sec.network.online')}</SectionTitle>
      <Card>
        <ToggleRow path="network.watch" title={t('network.watch.title')} desc={t('network.watch.desc')} />
        <Row title={t('network.login.title')} desc={t('network.login.desc')}>
          <ActionIcon name="network_open_login" icon="open" label={t('network.login.open')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.network.dns')}</SectionTitle>
      <Card>
        <Row title={t('network.dns.title')} desc={t('network.dns.desc')} />
        <SelectRow path="network.dns" title={t('network.dns.server')} options={DNS_CHOICES.map((v) => ({ value: v, label: t(`network.dns.${v}`) }))} />
        {n.dns === 'nextdns' ? <DnsField path="network.dns_nextdns" value={n.dns_nextdns} label={t('network.dns.nextdns.id')} hint={t('network.dns.nextdns.hint')} /> : null}
        {n.dns === 'custom' ? <DnsField path="network.dns_custom" value={n.dns_custom} label={t('network.dns.custom.url')} hint={t('network.dns.custom.hint')} /> : null}
        <Row title={t('network.dns.install')} desc={t('network.dns.warn')}>
          <div className="network-allow">
            <ActionIcon name="network_dns_install" icon="download" variant="prominent" label={t('network.dns.install')} disabled={dnsBlocked} />
            <ActionIcon name="network_dns_settings" icon="open" label={t('network.dns.settings')} />
          </div>
        </Row>
      </Card>

      <SectionTitle>{t('sec.network.anomaly')}</SectionTitle>
      <Card>
        <ToggleRow path="network.anomaly" title={t('network.anomaly.title')} desc={t('network.anomaly.desc')} when={on} />
        <Row title={t('network.anomaly.ignore.title')} desc={t('network.anomaly.ignore.desc')} when={on && n.anomaly}>
          {ignore.controls}
        </Row>
        <div className="row block">{ignore.chips}</div>
      </Card>

      <SectionTitle info={t('network.links.desc')}>{t('sec.network.links')}</SectionTitle>
      <Card>
        <div className="row block">
          <Links network={n} />
        </div>
      </Card>

      <SectionTitle>{t('sec.network.history')}</SectionTitle>
      <Card>
        <ToggleRow path="network.history" title={t('network.history.title')} desc={t('network.history.desc')} when={on} />
        <SelectRow
          path="network.history_seconds"
          title={t('network.history.pace.title')}
          desc={t('network.history.pace.desc')}
          when={on && n.history}
          options={[2, 5, 10].map((v) => ({ value: v, label: t('network.history.pace.n', { n: v }) }))}
        />
        <History network={n} on={on} />
        <Row title={t('network.history.export.title')} desc={t('network.history.export.desc')} when={on}>
          <ExportControls />
        </Row>
        <Row title={t('network.history.clear')} desc={t('network.history.clear.desc')} when={on}>
          <WipeButton />
        </Row>
      </Card>

      <SectionTitle>{t('sec.network.control')}</SectionTitle>
      <Card>
        <ToggleRow path="network.control" title={t('network.control.title')} desc={t('network.control.desc')} when={on} />
        <Row
          when={on}
          title={
            <span className="row-desc network-status" data-state={n.status || 'off'}>
              {n.paused && n.running ? t('network.paused') : t(`network.status.${n.status || 'off'}`)}
            </span>
          }
        />
        {!n.saved || (n.control && n.status === 'on') ? null : (
          <Row title={t('network.restore')} desc={t('network.restore.desc')}>
            <ActionIcon name="network_restore" icon="refresh" label={t('network.restore')} />
          </Row>
        )}
      </Card>

      <SectionTitle info={`${t('network.tether.desc')} ${t('network.tether.apps')} ${t('network.tether.iphone')}`}>{t('sec.network.tether')}</SectionTitle>
      <Card>
        <div className="row block">
          <Phone network={n} />
        </div>
      </Card>
    </>
  );
}
