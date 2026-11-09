import { useEffect } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import { Button } from '@/components/macos/Controls';
import { Card, SectionTitle } from '@/components/macos/Layout';
import { clamp, cx } from '@/lib/text';
import { send, useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { rate, size, sum } from './format';
import type { MoleState, Status } from './types';

const HISTORY = 60;
const KINDS = ['clean', 'purge', 'installers'] as const;

// The last readings of the live figures, kept across visits of the page. A reading is added once: the page may draw again
// for the same one.
const hist: { at: number | null; cpu: number[]; memory: number[]; gpu: number[]; down: number[]; up: number[] } = { at: null, cpu: [], memory: [], gpu: [], down: [], up: [] };

function remember(st: Status): void {
  if (st.at == null || st.at === hist.at) return;
  hist.at = st.at;
  const add = (k: 'cpu' | 'memory' | 'gpu' | 'down' | 'up', v: number | null | undefined) => {
    hist[k].push(+(v ?? 0) || 0);
    if (hist[k].length > HISTORY) hist[k].shift();
  };
  add('cpu', st.cpu);
  add('memory', st.memory);
  add('gpu', st.gpu);
  add('down', st.net_down);
  add('up', st.net_up);
}

/** A filled line of readings, oldest on the left, against `top` (or the largest of them). */
function Spark({ values, top, cls }: { values: number[]; top?: number; cls: string }) {
  const max = Math.max(top || 0, ...values, 1e-9);
  const step = 120 / Math.max(1, HISTORY - 1);
  const x0 = 120 - (values.length - 1) * step;
  const pts = values.map((v, i) => `${(x0 + i * step).toFixed(1)},${(36 - (v / max) * 33).toFixed(1)}`);
  return (
    <div className={cx('mole-spark', cls)} aria-hidden="true" hidden={pts.length < 2}>
      {pts.length < 2 ? null : (
        <svg viewBox="0 0 120 36" preserveAspectRatio="none">
          <path className="area" d={`M${x0.toFixed(1)},36L${pts.join('L')}L120,36Z`} />
          <path className="line" d={`M${pts.join('L')}`} />
        </svg>
      )}
    </div>
  );
}

/** A ring filled to `value` out of 100, its figure in the middle. */
function Gauge({ cls, label, value, text, hidden }: { cls: string; label: string; value: number | null | undefined; text?: string; hidden?: boolean }) {
  const n = clamp(+(value ?? 0) || 0, 0, 100);
  return (
    <div className={cx('mole-gauge', cls)} role="meter" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(n)} aria-label={label} style={{ '--v': n } as CSSProperties} hidden={hidden}>
      <b>{text ?? `${Math.round(n)}%`}</b>
      <span>{label}</span>
    </div>
  );
}

function Tile({ cls, title, value, sub, tone, children, hidden }: { cls: string; title: string; value: string; sub: string; tone?: string; children?: ReactNode; hidden?: boolean }) {
  return (
    <div className={cx('mole-tile', cls)} hidden={hidden}>
      <div className="mole-tile-head">
        <span>{title}</span>
        <b className="mole-tile-value">{value}</b>
      </div>
      <span className="mole-tile-sub" data-tone={tone ?? ''}>
        {sub}
      </span>
      {children}
    </div>
  );
}

function Stack({ parts, whole, big }: { parts: Array<[string, number]>; whole: number | undefined; big?: boolean }) {
  return (
    <div className={cx('mole-stack', big && 'big')}>
      {parts.map(([cls, n]) => (
        <span key={cls} className={cls} style={{ width: `${whole && whole > 0 ? (n / whole) * 100 : 0}%` }} />
      ))}
    </div>
  );
}

function Legend({ parts }: { parts: Array<[string, string]> }) {
  return (
    <div className="mole-legend">
      {parts.map(([cls, text]) => (
        <span key={cls} className={cls}>
          <i />
          {text}
        </span>
      ))}
    </div>
  );
}

function Fact({ label, value, share, title }: { label: string; value: string; share: number; title?: string }) {
  return (
    <div className="mole-fact" title={title}>
      <div className="mole-meter-label">
        <span className={title ? 'mole-top-name' : undefined}>{label}</span>
        <b>{value}</b>
      </div>
      <div className="mole-bar">
        <span style={{ width: `${clamp(share * 100, 0, 100)}%` }} />
      </div>
    </div>
  );
}

/** The health of the Mac: a score, four rings, and a tile each for the processor, the memory, the network, the disk, the heat
 *  and the busiest processes, with what can be freed. The status is read again each time a new reading has come in. */
export function Health({ mole }: { mole: MoleState }) {
  const t = useT();
  const s: Status = mole.status ?? ({} as Status);
  const busy = (mole.busy ?? []).includes('status');
  remember(s);

  // While the page is open the health is read again each time a new reading has come in, and at least every five seconds.
  useEffect(() => {
    let asked = (useApp.getState().state.mole as MoleState).status?.at ?? null;
    let askedAt = Date.now();
    const timer = setInterval(() => {
      const now = useApp.getState().state.mole as MoleState;
      if ((now.busy ?? []).includes('status') || document.visibilityState !== 'visible') return;
      const at = now.status?.at ?? null;
      if (at !== asked || Date.now() - askedAt > 5000) {
        asked = at;
        askedAt = Date.now();
        send({ t: 'action', name: 'mole_status' });
      }
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  const word = (n: number) => (n >= 85 ? 'good' : n >= 60 ? 'fair' : 'poor');
  const n = s.score;
  const lists = KINDS.map((k) => mole[k] ?? []);
  const can = lists.reduce((a, l) => a + sum(l), 0);
  const sizes = lists.map((l) => sum(l));
  const cores = s.cores ?? [];
  const slow = s.kinds ? s.kinds[0] : 0;

  const memParts: Array<[string, number]> = [['app', s.memory_app ?? 0], ['wired', s.memory_wired ?? 0], ['compressed', s.memory_compressed ?? 0]];
  const topNet = Math.max(1024, ...hist.down, ...hist.up);
  const other = Math.max(0, (s.disk_used || 0) - sizes.reduce((a, b) => a + b, 0));

  const facts: Array<[string, string, number]> = [];
  (s.fans ?? []).forEach((f, i) => facts.push([t('mole.fan', { n: i + 1 }), `${Math.round(f.rpm)} rpm`, f.max ? f.rpm / f.max : 0]));
  if (s.battery) facts.push([t('mole.battery'), `${s.battery.health}% / ${t('mole.battery.cycles', { n: s.battery.cycles })}`, s.battery.health / 100]);

  const tops = ['cpu', 'memory'].map((k, i) => {
    const list = (i ? s.top_memory : s.top_cpu) ?? [];
    const biggest = Math.max(1e-9, ...list.map((p) => (i ? p.memory : p.cpu)));
    return (
      <div className={cx('mole-top', k)} key={k}>
        <div className="mole-tile-head">
          <span>{t(`mole.top.${k}`)}</span>
        </div>
        <div className="mole-top-rows">
          {list.length ? (
            list.map((p) => {
              const v = i ? p.memory : p.cpu;
              return <Fact key={p.pid} title={`${p.name} (${p.pid})`} label={p.name} value={i ? size(v) : `${v.toFixed(1)}%`} share={clamp((v / biggest) * 100, 2, 100) / 100} />;
            })
          ) : (
            <div className="mole-tile-sub">{t('mole.top.none')}</div>
          )}
        </div>
      </div>
    );
  });

  return (
    <>
      <SectionTitle>{t('mole.sec.health')}</SectionTitle>
      <Card>
        <div className="mole-hero">
          <div
            className={cx('mole-score-ring', n != null && word(n))}
            role="meter"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={n ?? undefined}
            aria-label={t('mole.health.title')}
            style={{ '--v': n || 0 } as CSSProperties}
          >
            <b>{n != null ? String(n) : '--'}</b>
            <span>{t('mole.health.title')}</span>
          </div>
          <div className="mole-hero-text">
            <div className="mole-hero-word">{n == null ? t('mole.health.reading') : t(`mole.health.${word(n)}`)}</div>
            <div className="mole-hero-free">{can ? t('mole.health.can_free', { size: size(can) }) : ''}</div>
            <div className="mole-actions">
              <span className={cx('mole-live', hist.cpu.length > 1 && 'on')}>{t('mole.live')}</span>
              <Button small icon="refresh" disabled={busy} onClick={() => send({ t: 'action', name: 'mole_refresh' })}>
                {t('mole.refresh.all')}
              </Button>
            </div>
          </div>
          <div className="mole-gauges">
            <Gauge cls="cpu" label={t('mole.cpu')} value={s.cpu} />
            <Gauge cls="mem" label={t('mole.memory')} value={s.memory} />
            <Gauge cls="disk" label={t('mole.disk')} value={s.disk} />
            <Gauge cls="gpu" label={t('mole.gpu')} value={s.gpu} hidden={s.gpu == null} />
          </div>
        </div>

        <div className="mole-tiles">
          <Tile
            cls="cpu"
            title={t('mole.cpu')}
            value={`${Math.round(s.cpu || 0)}%`}
            sub={s.kinds ? t('mole.cores', { slow: s.kinds[0], fast: s.kinds[1] }) : cores.length ? t('mole.cores.n', { n: cores.length }) : ''}
          >
            <Spark cls="cpu" values={hist.cpu} top={100} />
            <div className="mole-cores">
              {cores.map((v, i) => (
                <span key={i} className={i < slow ? 'e' : 'p'}>
                  <i style={{ height: `${clamp(v, 2, 100)}%` }} />
                </span>
              ))}
            </div>
          </Tile>
          <Tile
            cls="mem"
            title={t('mole.memory')}
            value={s.memory_total ? `${size(s.memory_used ?? 0)} / ${size(s.memory_total)}` : ''}
            sub={s.pressure ? t(`mole.pressure.${s.pressure}`) + (s.swap_used ? ` / ${t('mole.swap', { used: size(s.swap_used), total: size(s.swap_total ?? 0) })}` : '') : ''}
            tone={s.pressure}
          >
            <Spark cls="mem" values={hist.memory} top={100} />
            <Stack parts={memParts} whole={s.memory_total} />
            <Legend parts={memParts.map(([k, v]) => [k, `${t(`mole.mem.${k}`)} ${size(v)}`])} />
          </Tile>
          <Tile cls="net" title={t('mole.network')} value={rate(s.net_down || 0)} sub={`${t('mole.up')} ${rate(s.net_up || 0)}`}>
            <Spark cls="down" values={hist.down} top={topNet} />
            <Spark cls="up" values={hist.up} top={topNet} />
            <Legend parts={[['down', `${t('mole.down')} ${rate(s.net_down || 0)}`], ['up', `${t('mole.up')} ${rate(s.net_up || 0)}`]]} />
          </Tile>
          <Tile
            cls="disk"
            title={t('mole.disk')}
            value={`${Math.round(s.disk || 0)}%`}
            sub={s.disk_total ? `${size(s.disk_used ?? 0)} / ${size(s.disk_total)} / ${t('mole.disk.free', { size: size(s.disk_total - (s.disk_used ?? 0)) })}` : ''}
          >
            <Stack whole={s.disk_total} parts={[['used', other], ...KINDS.map((k, i): [string, number] => [k, sizes[i]])]} />
          </Tile>
          <Tile
            cls="heat"
            title={t('mole.sensors')}
            value={s.celsius != null ? `${Math.round(s.celsius)}\u00b0C` : s.thermal ? t(`mole.thermal.${s.thermal}`) : ''}
            sub={s.celsius != null && s.thermal ? t(`mole.thermal.${s.thermal}`) : ''}
            tone={s.thermal}
            hidden={s.celsius == null && !facts.length && !s.thermal}
          >
            <div className="mole-facts">
              {facts.map(([label, value, share]) => (
                <Fact key={label} label={label} value={value} share={share} />
              ))}
            </div>
          </Tile>
          <div className="mole-tops">{tops}</div>
        </div>

        <div className="mole-free" hidden={!can}>
          <div className="mole-tile-head">
            <span>{t('mole.free.title')}</span>
            <b>{size(can)}</b>
          </div>
          <Stack big whole={can} parts={KINDS.map((k, i): [string, number] => [k, sizes[i]])} />
          <Legend parts={KINDS.map((k, i): [string, string] => [k, `${t(`mole.sec.${k}`)} ${size(sizes[i])}`])} />
        </div>
      </Card>
    </>
  );
}
