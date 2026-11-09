import { useEffect, useState } from 'react';
import { Button } from '@/components/macos/Controls';
import { cx } from '@/lib/text';
import { send } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { size, tilde } from './format';
import type { Analysis, Item, MoleState } from './types';
import { setChosen, setLit, useMoleUi } from './ui';

const act = (name: string) => send({ t: 'action', name });

const MAX_SLICES = 12;
const MIN_SHARE = 0.01;

// Shades of the accent, from the full colour to a pale one mixed with white, as the rest of the window has no other colour;
// white keeps the pale ones bright on the dark theme.
const PALETTE = [100, 86, 74, 62, 52, 43, 35].map((n) => `color-mix(in srgb, var(--tint) ${n}%, #ffffff)`);

/** The colour of the k-th row of the list and slice of the ring. */
export const tintOf = (k: number) => (k < MAX_SLICES - 1 ? PALETTE[k % PALETTE.length] : 'var(--text-2)');

const R_OUT = 90;
const R_IN = 58;
const point = (r: number, turn: number) => `${(100 + r * Math.sin(turn * 2 * Math.PI)).toFixed(2)} ${(100 - r * Math.cos(turn * 2 * Math.PI)).toFixed(2)}`;

function arc(from: number, to: number): string {
  const end = Math.min(to, from + 0.9999);
  const big = end - from > 0.5 ? 1 : 0;
  return `M${point(R_OUT, from)}A${R_OUT} ${R_OUT} 0 ${big} 1 ${point(R_OUT, end)}L${point(R_IN, end)}A${R_IN} ${R_IN} 0 ${big} 0 ${point(R_IN, from)}Z`;
}

interface Slice {
  entry: Item | null;
  name: string;
  size: number;
  share: number;
}

/** The slices of the ring: the biggest entries, then one "Other" for the rest and for what is too thin to see. */
function slices(a: Analysis, other: string): Slice[] {
  const list = [...a.entries].filter((i) => i.size > 0).sort((x, y) => y.size - x.size);
  const whole = Math.max(a.total || 0, list.reduce((n, i) => n + i.size, 0));
  if (!list.length || !whole) return [];
  const out: Array<Omit<Slice, 'share'>> = [];
  let rest = whole;
  for (const i of list) {
    if (out.length >= MAX_SLICES - 1 || i.size / whole < MIN_SHARE) break;
    out.push({ entry: i, name: i.name, size: i.size });
    rest -= i.size;
  }
  if (rest > 0) out.push({ entry: null, name: other, size: rest });
  return out.map((x) => ({ ...x, share: x.size / whole }));
}

/** The ring above the list. The list stays the way in for the keyboard, so the slices are not tab stops. */
function Chart({ mole }: { mole: MoleState }) {
  const t = useT();
  const a = mole.analyze;
  const busy = mole.busy.includes('analyze');
  const cut = a && a.entries && !busy ? slices(a, t('mole.analyze.other')) : [];
  const [hover, setHover] = useState<number | null>(null);
  const chosen = useMoleUi((s) => s.chosen.analyze) ?? [];

  // The row the ring points at is marked in the list; it is not marked once the ring goes away.
  const point_ = (k: number | null) => {
    setHover(k);
    setLit(k !== null && cut[k]?.entry ? cut[k].entry!.path : null);
  };
  useEffect(() => () => setLit(null), []);

  if (!a || !cut.length) return <div className="mole-chart" hidden />;
  const x = hover !== null ? cut[hover] : undefined;
  let at = 0;
  const top = cut
    .filter((s) => s.entry)
    .slice(0, 3)
    .map((s) => `${s.name} ${size(s.size)}`)
    .join(', ');
  const choose = (s: Slice) => {
    if (!s.entry) return;
    if (s.entry.dir) {
      act(`mole_analyze:${s.entry.path}`);
      return;
    }
    if (!chosen.includes(s.entry.path)) setChosen('analyze', [...chosen, s.entry.path]);
    const row = [...document.querySelectorAll<HTMLElement>('.mole-item')].find((r) => r.dataset.path === s.entry!.path);
    row?.scrollIntoView({ block: 'nearest' });
  };

  return (
    <div className="mole-chart">
      <svg viewBox="0 0 200 200" role="img" aria-label={t('mole.analyze.chart', { path: tilde(a.path, mole.home), total: size(a.total ?? 0), top })}>
        {cut.map((s, k) => {
          const d = arc(at, at + s.share);
          at += s.share;
          return (
            <path
              key={k}
              className={cx('mole-slice', s.entry && !s.entry.dir && 'file', hover === k && 'lit')}
              d={d}
              style={{ fill: s.entry ? PALETTE[k % PALETTE.length] : 'var(--text-2)' }}
              onMouseEnter={() => point_(k)}
              onMouseLeave={() => point_(null)}
              onClick={() => choose(s)}
            />
          );
        })}
        <foreignObject x="46" y="70" width="108" height="60">
          <div className="mole-chart-mid" aria-hidden="true">
            <span className="mole-chart-name">{x ? x.name : tilde(a.path, mole.home).split('/').pop() || '/'}</span>
            <span className="mole-chart-size">{x ? `${size(x.size)} / ${Math.round(x.share * 100)}%` : size(a.total ?? 0)}</span>
          </div>
        </foreignObject>
      </svg>
      <div className="mole-chart-legend" aria-hidden="true">
        {cut.map((s, k) => (
          <span key={k} className={cx('mole-legend-item mole-slice-key', hover === k && 'lit')} onMouseEnter={() => point_(k)} onMouseLeave={() => point_(null)} onClick={() => choose(s)}>
            <i style={{ background: s.entry ? PALETTE[k % PALETTE.length] : 'var(--text-2)' }} />
            {s.name}
            <em>{size(s.size)}</em>
          </span>
        ))}
      </div>
    </div>
  );
}

/** The folder being looked at, with each folder of its path a step back up to it, the home folder counted as one. */
export function AnalyzeHead({ mole }: { mole: MoleState }) {
  const t = useT();
  const a = mole.analyze;
  const busy = mole.busy.includes('analyze');
  const home = mole.home;
  const inHome = !!a && !!home && (a.path === home || a.path.startsWith(`${home}/`));
  const rest = a ? (inHome ? a.path.slice(home.length) : a.path).split('/').filter(Boolean) : [];
  const steps: Array<[string, string]> = a ? [[inHome ? home : '/', inHome ? '~' : '/'], ...rest.map((n, k): [string, string] => [`${inHome ? home : ''}/${rest.slice(0, k + 1).join('/')}`, n])] : [];
  return (
    <div>
      <Chart mole={mole} />
      <div className="mole-toolbar">
        <nav className="mole-path" aria-label={t('mole.sec.analyze')}>
          {steps.map(([path, name], k) =>
            k === steps.length - 1 ? (
              <b key={path}>{name}</b>
            ) : (
              <button key={path} className="mole-crumb" type="button" onClick={() => act(`mole_analyze:${path}`)}>
                {name}
              </button>
            ),
          )}
          {a ? <span className="mole-path-size">{a.total != null ? size(a.total) : t('mole.scanning')}</span> : null}
        </nav>
        <Button small icon="chevron.up" disabled={!a || a.path === '/' || busy} onClick={() => a && act(`mole_analyze:${a.path.replace(/\/[^/]+\/?$/, '') || '/'}`)}>
          {t('mole.analyze.up')}
        </Button>
        <Button small disabled={!a || a.path === home || busy} onClick={() => act(`mole_analyze:${home}`)}>
          {t('mole.analyze.home')}
        </Button>
      </div>
    </div>
  );
}
