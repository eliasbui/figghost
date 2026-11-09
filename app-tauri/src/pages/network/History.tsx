import { useState } from 'react';
import { ActionButton, Segmented, Select, SureButton } from '@/components/macos/Controls';
import { Row } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { linkName, size } from './helpers';
import type { HistoryRow, NetworkState } from './types';

type Range = 'today' | 'week' | 'month';

/** What each app moved over the days: the range, the table, the bars of each day, and the way out. */
export function History({ network, on }: { network: NetworkState; on: boolean }) {
  const t = useT();
  const [range, setRange] = useState<Range>('today');
  const view = network.history_view ?? {};
  const list: HistoryRow[] = view[range] ?? [];
  const daily = view.daily ?? [];
  const most = Math.max(1, ...daily.map((d) => d.down + d.up));
  const viaOf = (a: HistoryRow) => (a.interfaces ?? []).map((i) => (i.name === '?' ? t('network.history.unknown') : linkName(network, i.name))).join(', ') || '-';
  return (
    <>
      <Row
        when={on}
        title={
          <Segmented
            label={t('sec.network.history')}
            value={range}
            options={(['today', 'week', 'month'] as Range[]).map((k) => ({ value: k, label: t(`network.history.range.${k}`) }))}
            onPick={setRange}
          />
        }
      />
      <div className="row block network-table-row">
        <div className="network-table network-history-table" role="table">
          {list.length ? (
            <>
              <div className="network-tr head" role="row">
                {['app', 'down', 'up', 'total', 'via'].map((k) => (
                  <span key={k} role="columnheader">
                    {t(k === 'total' ? 'network.history.col.total' : `network.col.${k}`)}
                  </span>
                ))}
              </div>
              {list.map((a) => (
                <div className="network-tr" role="row" key={a.app}>
                  <span className="network-app">{a.app}</span>
                  <span>{size(a.down)}</span>
                  <span>{size(a.up)}</span>
                  <span>{size(a.down + a.up)}</span>
                  <span className="network-via">{viaOf(a)}</span>
                </div>
              ))}
            </>
          ) : (
            <div className="row-desc network-empty">{t('network.history.none')}</div>
          )}
        </div>
      </div>
      <div className="row block network-bars-row">
        <div className="row-desc" hidden={view.empty !== false}>
          {t('network.history.daily')}
        </div>
        <div className="network-bars" role="img" aria-label={t('network.history.daily')} hidden={view.empty !== false}>
          {daily.map((d) => (
            <span className="network-bar" key={d.day} title={`${d.day}: ${size(d.down)} / ${size(d.up)}`}>
              <span className="network-bar-fill" style={{ height: `${Math.max(2, Math.round(((d.down + d.up) / most) * 100))}%` }} />
              <span className="network-bar-day">{d.day.slice(0, 2)}</span>
            </span>
          ))}
        </div>
      </div>
    </>
  );
}

/** The format, the number of days and the grain of an export, and the button that asks for it. */
export function ExportControls() {
  const t = useT();
  const [format, setFormat] = useState<string>('csv');
  const [days, setDays] = useState<number>(7);
  const [grain, setGrain] = useState<string>('day');
  return (
    <div className="network-export">
      <Select label="format" value={format} options={[{ value: 'csv', label: 'CSV' }, { value: 'json', label: 'JSON' }]} onChange={(v) => setFormat(String(v))} />
      <Select label="days" value={days} options={[7, 30, 90].map((n) => ({ value: n, label: t('network.history.export.days', { n }) }))} onChange={(v) => setDays(Number(v))} />
      <Select label="grain" value={grain} options={['day', 'month'].map((k) => ({ value: k, label: t(`network.history.export.${k}`) }))} onChange={(v) => setGrain(String(v))} />
      <ActionButton name={`network_history_export:${format}:${days}:${grain}`} primary>
        {t('network.history.export.go')}
      </ActionButton>
    </div>
  );
}

/** Clearing the history asks twice. */
export function WipeButton() {
  const t = useT();
  return <SureButton label={t('network.history.clear')} sure={t('network.history.clear.sure')} onConfirm={() => send({ t: 'action', name: 'network_history_clear' })} />;
}
