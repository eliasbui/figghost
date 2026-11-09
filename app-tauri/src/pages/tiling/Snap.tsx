import { useEffect, useState } from 'react';
import { create } from 'zustand';
import { Button, IconButton, Select, ToggleRow } from '@/components/macos/Controls';
import { confirmDialog } from '@/components/macos/Dialogs';
import { Card, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { Icon } from '@/components/macos/Icon';
import { InfoTip } from '@/components/macos/Overlay';
import { cx } from '@/lib/text';
import { change, patch, send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './snap.css';

// Dragging a window onto the notch to snap it into a layout. Cards at the end of "Window snapping" hold the switch, the
// Accessibility state, the choice of layouts for each screen and the layouts a person makes from a grid.

// core-window's snap::PER_ROW, templates::MAX_GRID, MAX_CUSTOM and MAX_NAME, and templates::DEFAULT.
const PER_ROW = 10;
const MAX_GRID = 6;
const MAX_CUSTOM = 12;
const MAX_NAME = 24;
const DEFAULT = ['full', 'halves', 'thirds', 'quarters', 'rows', 'main_left', 'main_right'];

/** A cell as [column, row, column after, row after] of a grid. */
type Region = number[];

interface Custom {
  id: string;
  name: string;
  cols: number;
  rows: number;
  cells: Region[];
}
interface SnapState {
  enabled: boolean;
  accessibility: boolean;
  built_in: Array<{ id: string; cells: number[][] }>;
  custom: Custom[];
  templates: string[];
  screens: Record<string, string[]>;
  displays: string[];
}
interface Editing {
  id: string;
  name: string;
  cols: number;
  rows: number;
  regions: Region[];
  picked: number[];
}

// The screen the list is for ('' for all the screens that have none of their own), and the layout being made or changed.
// They outlive the cards, which are drawn again each time the page is opened.
const useUi = create<{ screen: string; editing: Editing | null }>()(() => ({ screen: '', editing: null }));
const edit = (next: Partial<Editing> | null) =>
  useUi.setState((s) => ({ editing: next === null ? null : { ...(s.editing as Editing), ...next } }));

// -- what the state holds

const catalog = (snap: SnapState) => [...snap.built_in.map((b) => b.id), ...snap.custom.map((c) => c.id)];

/** The cells of a layout as fractions of the screen: x0, y0, x1, y1. */
function cellsOf(snap: SnapState, id: string): number[][] {
  const built = snap.built_in.find((b) => b.id === id);
  if (built) return built.cells;
  const mine = snap.custom.find((c) => c.id === id);
  return mine ? mine.cells.map(([c0, r0, c1, r1]) => [c0 / mine.cols, r0 / mine.rows, c1 / mine.cols, r1 / mine.rows]) : [];
}

/** The ids chosen for the screen being looked at. */
const chosenFor = (snap: SnapState, screen: string) => (screen && snap.screens[screen] ? snap.screens[screen] : snap.templates);

/** The screens that have a list of their own or are connected now. */
const screensOf = (snap: SnapState) => [...new Set([...snap.displays, ...Object.keys(snap.screens)])];

function write(snap: SnapState, screen: string, list: string[]): void {
  if (!screen) change('snap.templates', list);
  else change('snap.screens', { ...snap.screens, [screen]: list });
}

// -- drawing a layout

/** A small screen with the cells of a layout. */
function Mini({ cells, big }: { cells: number[][]; big?: boolean }) {
  return (
    <div className={cx('snap-mini', big && 'big')} aria-hidden="true">
      {cells.map(([x0, y0, x1, y1], i) => (
        <i
          key={i}
          style={{
            left: `calc(${x0 * 100}% + .75px)`,
            top: `calc(${y0 * 100}% + .75px)`,
            width: `calc(${(x1 - x0) * 100}% - 1.5px)`,
            height: `calc(${(y1 - y0) * 100}% - 1.5px)`,
          }}
        />
      ))}
    </div>
  );
}

/** How many layouts go on each row of the row under the notch: as even as can be, none more than PER_ROW. */
function rowLengths(count: number): number[] {
  if (count < 1) return [];
  const rows = Math.ceil(count / PER_ROW);
  const base = Math.floor(count / rows);
  const extra = count % rows;
  return Array.from({ length: rows }, (_, i) => base + (i < extra ? 1 : 0));
}

function useNameOf() {
  const t = useT();
  return (snap: SnapState, id: string) => snap.custom.find((c) => c.id === id)?.name ?? t(`snap.t.${id}`);
}

/** The layouts as they will hang from the notch: a dark shape with the layouts in rows, each a small screen with its name. */
function Preview({ snap, ids }: { snap: SnapState; ids: string[] }) {
  const t = useT();
  const nameOf = useNameOf();
  let at = 0;
  return (
    <div className="snap-stage">
      <div className="snap-shape" role="img" aria-label={`${t('snap.preview')}: ${ids.map((id) => nameOf(snap, id)).join(', ')}`}>
        {rowLengths(ids.length).map((n, row) => {
          const keys = ids.slice(at, (at += n));
          return (
            <div className="snap-row" key={row}>
              {keys.map((id) => (
                <span className="snap-cell" key={id}>
                  <Mini cells={cellsOf(snap, id)} />
                  <span className="snap-cap">{nameOf(snap, id)}</span>
                </span>
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}

// -- choosing the layouts of a screen

/** The choices, one button for each layout. The last layout cannot be removed; the button says so instead of ignoring the click. */
function PickList({ snap, screen }: { snap: SnapState; screen: string }) {
  const t = useT();
  const nameOf = useNameOf();
  const chosen = chosenFor(snap, screen);
  return (
    <div className="snap-picks" role="group" aria-label={t('snap.layouts.title')}>
      {catalog(snap).map((id) => {
        const on = chosen.includes(id);
        return (
          <button
            key={id}
            type="button"
            className="snap-pick"
            aria-pressed={on}
            disabled={on && chosen.length <= 1}
            onClick={() => {
              const next = catalog(snap).filter((k) => (k === id ? !on : chosen.includes(k)));
              if (next.length >= 1) write(snap, screen, next);
            }}
          >
            <span className="snap-pick-mini">
              <Mini cells={cellsOf(snap, id)} />
            </span>
            <span className="snap-name">{nameOf(snap, id)}</span>
          </button>
        );
      })}
    </div>
  );
}

function LayoutsCard({ snap }: { snap: SnapState }) {
  const t = useT();
  const screen = useUi((s) => s.screen);
  const screens = screensOf(snap);
  // A screen that went away and has no list of its own is no longer one to look at.
  useEffect(() => {
    if (screen && !screens.includes(screen)) useUi.setState({ screen: '' });
  }, [screen, screens.join('\n')]);
  const chosen = chosenFor(snap, screen);
  const own = !!screen && !!snap.screens[screen];
  const options = [
    { value: '', label: t('snap.screen.all') },
    ...screens.map((name) => ({ value: name, label: snap.displays.includes(name) ? name : t('snap.screen.away', { name }) })),
  ];
  return (
    <Card className={cx(!snap.enabled && 'dim')}>
      <Row title={t('snap.screen.title')} desc={t('snap.layouts.desc')}>
        <Select label={t('snap.screen.title')} value={screen} options={options} onChange={(v) => useUi.setState({ screen: String(v) })} />
      </Row>
      <div className="snap-body">
        <Preview snap={snap} ids={chosen} />
        <PickList snap={snap} screen={screen} />
        <div className="snap-foot">
          <div>
            <div className="row-desc snap-count" role="status">
              {t('snap.layouts.count', { n: chosen.length })}
              <InfoTip>{!screen ? t('snap.screen.note.all') : own ? t('snap.screen.note.own') : t('snap.screen.note.shared')}</InfoTip>
            </div>
          </div>
          <div className="snap-actions">
            {own ? (
              <Button
                small
                icon="refresh"
                onClick={() => {
                  const rest = { ...snap.screens };
                  delete rest[screen];
                  change('snap.screens', rest);
                }}
              >
                {t('snap.layouts.follow')}
              </Button>
            ) : null}
            <Button small icon="refresh" onClick={() => write(snap, screen, DEFAULT.slice())}>
              {t('snap.layouts.reset')}
            </Button>
          </div>
        </div>
      </div>
    </Card>
  );
}

// -- layouts made from a grid

/** The parts of a grid of `cols` by `rows`, each as [column, row, column after, row after]. */
const base = (cols: number, rows: number): Region[] =>
  Array.from({ length: cols * rows }, (_, i) => [i % cols, Math.floor(i / cols), (i % cols) + 1, Math.floor(i / cols) + 1]);

const inReadingOrder = (regions: Region[]) => regions.slice().sort((a, b) => a[1] - b[1] || a[0] - b[0]);

/** Joins the picked regions into one: the smallest rectangle that holds them, grown until it cuts no other region in two.
 *  The others inside it go; the rest stay. */
function join(regions: Region[], picked: number[]): Region[] {
  let [c0, r0, c1, r1] = [Infinity, Infinity, 0, 0];
  for (const i of picked) {
    const g = regions[i];
    c0 = Math.min(c0, g[0]);
    r0 = Math.min(r0, g[1]);
    c1 = Math.max(c1, g[2]);
    r1 = Math.max(r1, g[3]);
  }
  for (let grew = true; grew; ) {
    grew = false;
    for (const g of regions) {
      const touches = g[0] < c1 && g[2] > c0 && g[1] < r1 && g[3] > r0;
      if (touches && (g[0] < c0 || g[1] < r0 || g[2] > c1 || g[3] > r1)) {
        c0 = Math.min(c0, g[0]);
        r0 = Math.min(r0, g[1]);
        c1 = Math.max(c1, g[2]);
        r1 = Math.max(r1, g[3]);
        grew = true;
      }
    }
  }
  const inside = (g: Region) => g[0] >= c0 && g[1] >= r0 && g[2] <= c1 && g[3] <= r1;
  return inReadingOrder([...regions.filter((g) => !inside(g)), [c0, r0, c1, r1]]);
}

/** Takes one joined region apart into the parts of the grid it covers. */
function split(regions: Region[], i: number): Region[] {
  const [c0, r0, c1, r1] = regions[i];
  const parts = base(c1 - c0, r1 - r0).map(([a, b, c, d]) => [a + c0, b + r0, c + c0, d + r0]);
  return inReadingOrder([...regions.filter((_, k) => k !== i), ...parts]);
}

function nextId(snap: SnapState): string {
  for (let n = 1; n < 10000; n += 1) if (!snap.custom.some((c) => c.id === `c${n}`)) return `c${n}`;
  return 'c1';
}

const startEditing = (layout: Custom | null) =>
  useUi.setState({
    editing: layout
      ? { id: layout.id, name: layout.name, cols: layout.cols, rows: layout.rows, regions: inReadingOrder(layout.cells.map((c) => c.slice())), picked: [] }
      : { id: '', name: '', cols: 2, rows: 1, regions: base(2, 1), picked: [] },
  });

function Editor({ snap, editing }: { snap: SnapState; editing: Editing }) {
  const t = useT();
  const screen = useUi((s) => s.screen);
  // The line the editor says what is wrong in; it goes when the name is typed.
  const [problem, setProblem] = useState('');
  const joinable = editing.picked.length >= 2;
  const splittable =
    editing.picked.length === 1 &&
    (() => {
      const g = editing.regions[editing.picked[0]];
      return g[2] - g[0] > 1 || g[3] - g[1] > 1;
    })();

  const save = () => {
    const name = editing.name.trim();
    if (!name) {
      setProblem(t('snap.edit.name.needed'));
      return;
    }
    const layout: Custom = { id: editing.id || nextId(snap), name, cols: editing.cols, rows: editing.rows, cells: inReadingOrder(editing.regions) };
    const fresh = !editing.id;
    change('snap.custom', fresh ? [...snap.custom, layout] : snap.custom.map((c) => (c.id === layout.id ? layout : c)));
    // A layout just made is offered where the list being looked at is, so that it shows at the next drag.
    if (fresh) {
      const after = { ...snap, custom: [...snap.custom, layout] };
      write(after, screen, catalog(after).filter((k) => k === layout.id || chosenFor(snap, screen).includes(k)));
    }
    edit(null);
  };

  const size = (key: 'cols' | 'rows', label: string) => (
    <label className="snap-size">
      <span>{label}</span>
      <Select
        small
        label={label}
        value={editing[key]}
        options={Array.from({ length: MAX_GRID }, (_, i) => ({ value: i + 1, label: String(i + 1) }))}
        onChange={(v) => {
          const n = Number(v);
          edit(key === 'cols' ? { cols: n, regions: base(n, editing.rows), picked: [] } : { rows: n, regions: base(editing.cols, n), picked: [] });
        }}
      />
    </label>
  );

  return (
    <div className="snap-editor">
      <div className="snap-edit-head">
        <div className="row-title">{editing.id ? t('snap.edit.title.change') : t('snap.edit.title.new')}</div>
      </div>
      <div className="snap-edit-fields">
        <input
          className="text-field"
          type="text"
          maxLength={MAX_NAME}
          value={editing.name}
          placeholder={t('snap.edit.name.ph')}
          aria-label={t('snap.edit.name')}
          spellCheck={false}
          autoComplete="off"
          onChange={(e) => {
            edit({ name: e.target.value });
            setProblem('');
          }}
        />
        {size('cols', t('snap.edit.cols'))}
        {size('rows', t('snap.edit.rows'))}
      </div>
      <div className="snap-grid" style={{ gridTemplateColumns: `repeat(${editing.cols},1fr)`, gridTemplateRows: `repeat(${editing.rows},1fr)` }}>
        {editing.regions.map((g, i) => (
          <button
            key={g.join(',')}
            type="button"
            className="snap-region"
            aria-pressed={editing.picked.includes(i)}
            aria-label={t('snap.edit.cell', { n: i + 1 })}
            style={{ gridColumn: `${g[0] + 1}/${g[2] + 1}`, gridRow: `${g[1] + 1}/${g[3] + 1}` }}
            onClick={() => edit({ picked: editing.picked.includes(i) ? editing.picked.filter((k) => k !== i) : [...editing.picked, i] })}
          >
            {i + 1}
          </button>
        ))}
      </div>
      <div className="snap-edit-tools">
        <Button small disabled={!joinable} onClick={() => edit({ regions: join(editing.regions, editing.picked), picked: [] })}>
          {t('snap.edit.join')}
        </Button>
        <Button small disabled={!splittable} onClick={() => edit({ regions: split(editing.regions, editing.picked[0]), picked: [] })}>
          {t('snap.edit.split')}
        </Button>
      </div>
      <div className="row-desc">{t('snap.edit.hint')}</div>
      <div className="snap-problem" role="alert">
        {problem}
      </div>
      <div className="snap-edit-foot">
        <Button onClick={() => edit(null)}>{t('snap.edit.cancel')}</Button>
        <Button primary onClick={save}>
          {t('snap.edit.save')}
        </Button>
      </div>
    </div>
  );
}

function MineRow({ snap, layout }: { snap: SnapState; layout: Custom }) {
  const t = useT();
  const remove = () =>
    confirmDialog({ title: t('snap.delete.title'), body: t('snap.delete.body'), confirm: t('snap.mine.delete') }, () => {
      change('snap.custom', snap.custom.filter((c) => c.id !== layout.id));
      // The app takes it off every list too; the page follows without waiting for the answer.
      const keep = (ids: string[]) => ids.filter((k) => k !== layout.id);
      patch('snap.templates', keep(snap.templates).length ? keep(snap.templates) : DEFAULT.slice());
      const screens: Record<string, string[]> = {};
      for (const [name, ids] of Object.entries(snap.screens)) if (keep(ids).length) screens[name] = keep(ids);
      patch('snap.screens', screens);
    });
  return (
    <div className="snap-mine">
      <Mini cells={cellsOf(snap, layout.id)} big />
      <div className="snap-mine-name">
        <div className="row-title">{layout.name}</div>
        <div className="row-desc">{t('snap.mine.cells', { n: layout.cells.length })}</div>
      </div>
      <div className="snap-actions">
        <Button small onClick={() => startEditing(layout)}>
          {t('snap.mine.edit')}
        </Button>
        <IconButton icon="trash" label={`${t('snap.mine.delete')}: ${layout.name}`} onClick={remove} />
      </div>
    </div>
  );
}

/** The card of the layouts a person made: the list, or the editor in its place. */
function MineCard({ snap }: { snap: SnapState }) {
  const t = useT();
  const editing = useUi((s) => s.editing);
  return (
    <Card>
      <Row title={t('snap.mine.title')} desc={t('snap.mine.desc')} />
      <div className="snap-body">
        {editing ? (
          <Editor snap={snap} editing={editing} />
        ) : (
          <>
            {snap.custom.length ? (
              <div className="snap-mines">
                {snap.custom.map((c) => (
                  <MineRow key={c.id} snap={snap} layout={c} />
                ))}
              </div>
            ) : (
              <div className="row-desc">{t('snap.mine.none')}</div>
            )}
            <div className="snap-foot">
              <div className="row-desc">{t('snap.mine.count', { n: snap.custom.length, max: MAX_CUSTOM })}</div>
              <Button small disabled={snap.custom.length >= MAX_CUSTOM} onClick={() => startEditing(null)}>
                <Icon name="plus" />
                {t('snap.mine.new')}
              </Button>
            </div>
          </>
        )}
      </div>
    </Card>
  );
}

export function SnapCards() {
  const t = useT();
  const snap = useSection<SnapState>('snap');
  return (
    <>
      <SectionTitle>{t('sec.snap')}</SectionTitle>
      <Card>
        <ToggleRow path="snap.enabled" title={t('snap.enable.title')} desc={t('snap.enable.desc')} />
        <Row title={t('snap.access.title')} desc={t('snap.access.desc')}>
          <Pill good={snap.accessibility}>{t(snap.accessibility ? 'snap.access.ok' : 'snap.access.no')}</Pill>
          {snap.accessibility ? null : (
            <Button small onClick={() => send({ t: 'action', name: 'open_accessibility' })}>
              {t('snap.access.open')}
            </Button>
          )}
        </Row>
      </Card>
      <LayoutsCard snap={snap} />
      <MineCard snap={snap} />
    </>
  );
}
