// The diagram of Network, in a window of its own: connections as circles, apps as boxes, all moved freely on a dotted grid;
// one connection wired to another makes the Combined circle around both. It is the drawing of the page this comes from,
// kept in the form it had (a DOM built by hand and moved by pointer events), since every part of it is a gesture.
import { send, tr, useApp } from '@/stores/app';
import { linkName, linkSymbol, size, speed } from '../helpers';
import type { NetworkState } from '../types';
import { button, h, iconEl, iconImg, refreshIcons } from './dom';

type Loose = Record<string, any>;

interface Place {
  x: number;
  y: number;
  size?: number;
  d?: number;
  rel?: { x: number; y: number };
  inside?: boolean;
}
type At = Record<string, Place>;
interface Model {
  links: Loose[];
  merge: string[];
  apps: Loose[];
}
interface Flow {
  path: SVGPathElement;
  down: number;
  up: number;
  width: number;
  cls: string;
  q: string;
  len: number;
  dots: Array<{ c: SVGCircleElement; dir: number; gap: number; v: number; i: number }>;
}

const NS = 'http://www.w3.org/2000/svg';
// D: a connection on its own; C: one inside the Combined circle, smaller so that it can move about in there.
const GRID = 20;
const D = 96;
const C = 70;
const APP_W = 184;
const APP_H = 54;
const MERGE_MIN = 200;
const MERGE_MAX = 640;
const Z_MIN = 0.3;
const Z_MAX = 2.5;
// Within this many pixels of the screen of a node's rim counts as its rim, whatever the zoom.
const RIM = 14;
const RANK: Record<string, number> = { good: 3, fair: 2, poor: 1, down: 0 };

// A connection inside the circle keeps its middle this far within the circle's edge.
function inCircle(rel: { x: number; y: number }, circle: number): { x: number; y: number } {
  const r = circle / 2 - C / 2 - 8;
  const cx = rel.x + C / 2 - circle / 2;
  const cy = rel.y + C / 2 - circle / 2;
  const d = Math.hypot(cx, cy);
  const k = d > r ? r / d : 1;
  return { x: Math.round(circle / 2 + cx * k - C / 2), y: Math.round(circle / 2 + cy * k - C / 2) };
}

const snap = (v: number) => Math.max(0, Math.round(v / GRID) * GRID);

/** Builds the diagram in `host` and keeps it in step with the state; returns the way to take it down. */
export function mountNetworkCanvas(host: HTMLElement): () => void {
  const N = () => useApp.getState().state.network as NetworkState & Loose;
  const send1 = (path: string, value: unknown) => send({ t: 'set', path, value });
  const stops: Array<() => void> = [];
  let alive = true;

  const head = h('header', { class: 'network-canvas-head glass', 'data-tauri-drag-region': 'deep' });
  const title = h('h1', { text: tr('page.network') });
  const totals = h('span', { class: 'network-total-head' });
  const note = h('span', { class: 'network-canvas-note' });
  const close = button('', () => send({ t: 'action', name: 'network_canvas_close' }), { icon: 'xmark', aria: tr('network.close') });
  // Arrange is a one-shot: the choice lays the boxes out, then the menu goes back to its title.
  const arrangeSel = h('select', { class: 'select network-arrange', 'aria-label': tr('network.arrange') }, h('option', { value: '', text: tr('network.arrange') }), ...['link', 'traffic', 'name'].map((k) => h('option', { value: k, text: tr(`network.arrange.${k}`) })));
  arrangeSel.addEventListener('change', () => {
    if (arrangeSel.value) arrange(arrangeSel.value);
    arrangeSel.value = '';
  });
  const zoomBtn = button('100%', () => zoomTo(1));
  zoomBtn.title = tr('network.zoom.reset');
  const fitBtn = button('', () => fit(), { icon: 'fit', aria: tr('network.zoom.fit') });
  head.append(title, totals, note, arrangeSel, fitBtn, zoomBtn, close);
  const scroller = h('div', { class: 'network-canvas-scroll' });
  const board = h('div', { class: 'network-canvas-board' });
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('class', 'network-wires');
  const layer = h('div', { class: 'network-nodes' });
  board.append(svg, layer);
  scroller.append(board);
  const detail = h('div', { class: 'network-card floating', hidden: true });
  const root = h('div', { class: 'network-canvas' }, head, scroller, detail);
  host.replaceChildren(root);

  // The view: the board is moved and scaled inside the window; the dotted grid on the window follows it.
  const view = { x: 0, y: 56, z: 1 };
  function applyView() {
    board.style.transform = `translate(${view.x}px, ${view.y}px) scale(${view.z})`;
    scroller.style.backgroundSize = `${GRID * view.z}px ${GRID * view.z}px`;
    scroller.style.backgroundPosition = `${view.x}px ${view.y}px`;
    zoomBtn.textContent = `${Math.round(view.z * 100)}%`;
  }
  // Zooms about a point of the window, so what is under the pointer stays under it.
  function zoomAt(z: number, px: number, py: number) {
    const nz = Math.min(Z_MAX, Math.max(Z_MIN, z));
    view.x = px - ((px - view.x) * nz) / view.z;
    view.y = py - ((py - view.y) * nz) / view.z;
    view.z = nz;
    applyView();
  }
  function zoomTo(z: number) {
    zoomAt(z, scroller.clientWidth / 2, scroller.clientHeight / 2);
  }
  // Everything on the board in sight, never larger than 100%.
  function fit() {
    if (!current) return;
    let x0 = Infinity;
    let y0 = Infinity;
    let x1 = -Infinity;
    let y1 = -Infinity;
    for (const [id, p] of Object.entries(current.at)) {
      const w = p.size || p.d || (id.startsWith('app:') ? APP_W : D);
      const hh = p.size || p.d || (id.startsWith('app:') ? APP_H : D);
      x0 = Math.min(x0, p.x);
      y0 = Math.min(y0, p.y);
      x1 = Math.max(x1, p.x + w);
      y1 = Math.max(y1, p.y + hh);
    }
    if (!isFinite(x0)) return;
    const W = scroller.clientWidth || 900;
    const H = scroller.clientHeight || 600;
    const pad = 40;
    // The header floats over the top of the window: the boxes are fitted under it.
    const top = 56;
    const z = Math.min(1, Math.max(Z_MIN, Math.min((W - pad * 2) / (x1 - x0), (H - top - pad * 2) / (y1 - y0))));
    view.z = z;
    view.x = (W - (x1 - x0) * z) / 2 - x0 * z;
    view.y = top + (H - top - (y1 - y0) * z) / 2 - y0 * z;
    applyView();
  }
  // Control and the wheel (a pinch on a trackpad comes as the same) zooms; the wheel alone moves the view.
  scroller.addEventListener(
    'wheel',
    (e) => {
      e.preventDefault();
      const b = scroller.getBoundingClientRect();
      if (e.ctrlKey || e.metaKey) zoomAt(view.z * Math.exp(-e.deltaY * 0.01), e.clientX - b.left, e.clientY - b.top);
      else {
        view.x -= e.deltaX;
        view.y -= e.deltaY;
        applyView();
      }
    },
    { passive: false },
  );
  // Space held, the middle button, or a drag on the empty board moves the view; nodes are left alone meanwhile.
  let spaceDown = false;
  scroller.addEventListener(
    'pointerdown',
    (e) => {
      const onNode = e.target instanceof Element && e.target.closest('[data-node]');
      if (!(spaceDown || e.button === 1 || (e.button === 0 && !onNode))) return;
      e.preventDefault();
      e.stopPropagation();
      const start = { x: e.clientX - view.x, y: e.clientY - view.y };
      scroller.classList.add('panning');
      scroller.setPointerCapture(e.pointerId);
      const move = (ev: PointerEvent) => {
        view.x = ev.clientX - start.x;
        view.y = ev.clientY - start.y;
        applyView();
      };
      const up = () => {
        scroller.classList.remove('panning');
        scroller.removeEventListener('pointermove', move);
        scroller.removeEventListener('pointerup', up);
        scroller.removeEventListener('pointercancel', up);
      };
      scroller.addEventListener('pointermove', move);
      scroller.addEventListener('pointerup', up);
      scroller.addEventListener('pointercancel', up);
    },
    true,
  );
  const typing = (e: Event) => e.target instanceof Element && e.target.closest('input, select, textarea, button');
  function onKey(e: KeyboardEvent) {
    if (e.code === 'Space' && !typing(e)) {
      e.preventDefault();
      spaceDown = e.type === 'keydown';
      scroller.classList.toggle('space', spaceDown);
      return;
    }
    if (e.type !== 'keydown' || !(e.ctrlKey || e.metaKey)) return;
    if (e.key === '=' || e.key === '+') {
      e.preventDefault();
      zoomTo(view.z * 1.2);
    } else if (e.key === '-') {
      e.preventDefault();
      zoomTo(view.z / 1.2);
    } else if (e.key === '0') {
      e.preventDefault();
      zoomTo(1);
    }
  }

  // Where each node is: what the person put there (kept in the settings), else a place of its own on the grid.
  const local = new Map<string, { x: number; y: number }>();
  const placed = (id: string): { x: number; y: number } | undefined => {
    const kept = local.get(id);
    if (kept) return kept;
    const saved = (N().layout ?? {})[id];
    return saved ? { x: saved[0], y: saved[1] } : undefined;
  };
  const els = new Map<string, HTMLElement>();
  let picked: string | null = null;
  let moving: string | null = null;
  let wiring: { id: string; line: SVGPathElement | null } | null = null;
  const told = new Set<string>();
  let current: { m: Model; at: At } | null = null;

  const model = (): Model => {
    const n = N();
    const live: Loose[] = n.links ?? [];
    const links: Loose[] = [];
    for (const k of n.known_links ?? []) links.push({ name: k.name, label: k.label, away: true });
    for (const l of live) {
      const at = links.find((x) => x.name === l.name);
      if (at) Object.assign(at, l, { away: false });
      else links.push({ ...l, away: false });
      // A connection seen is kept on the diagram, also while it is away.
      const known = (n.known_links ?? []).find((k) => k.name === l.name);
      if ((!known || known.label !== l.label) && !told.has(l.name + l.label)) {
        told.add(l.name + l.label);
        send1('network.known_link', { name: l.name, label: l.label });
      }
    }
    const merge = (n.merge ?? []).filter((m) => links.some((l) => l.name === m));
    const apps: Loose[] = [];
    const add = (name: string | undefined, bundle?: string) => {
      if (name && !apps.some((a) => a.name.toLowerCase() === name.toLowerCase())) apps.push({ name, bundle: bundle || '', down: 0, up: 0, interfaces: [], idle: true });
    };
    for (const k of n.known_apps ?? []) add(k.name, k.bundle);
    for (const a of n.apps ?? []) add(a.name);
    for (const w of n.wires ?? []) add(w.app);
    for (const a of apps) {
      const now = (n.apps ?? []).find((x) => x.name.toLowerCase() === a.name.toLowerCase());
      if (now) Object.assign(a, now, { bundle: now.bundle || a.bundle, idle: !(now.down || now.up) && !(now.interfaces ?? []).length });
      a.wire = (n.wires ?? []).find((w) => w.app.toLowerCase() === a.name.toLowerCase());
    }
    return { links, merge: merge.length >= 2 ? merge : [], apps };
  };

  // Places: the circles down the left, the apps in columns to their right, each until it is moved.
  const layout = (m: Model): At => {
    const at: At = {};
    let y = 40;
    if (m.merge.length) {
      const kept = placed('merge:size');
      const circle = Math.min(MERGE_MAX, Math.max(MERGE_MIN, kept ? kept.x : m.merge.length * (C + 16) + 90));
      at.merge = { ...(placed('merge') ?? { x: 40, y }), size: circle };
      y += circle + 40;
      m.merge.forEach((name, i) => {
        // Where it was put inside the circle, else in a row across its middle.
        const rel = inCircle(placed(`in:${name}`) ?? { x: circle / 2 + (i - (m.merge.length - 1) / 2) * (C + 16) - C / 2, y: circle / 2 - C / 2 + 6 }, circle);
        at[`link:${name}`] = { x: at.merge.x + rel.x, y: at.merge.y + rel.y, rel, inside: true, d: C };
      });
    }
    for (const l of m.links) {
      if (m.merge.includes(l.name)) continue;
      at[`link:${l.name}`] = placed(`link:${l.name}`) ?? { x: 60, y };
      y += D + 50;
    }
    // An app not moved yet takes the next cell of the columns that no box already covers.
    const boxes: Array<{ x: number; y: number }> = [];
    for (const a of m.apps) {
      const p = placed(`app:${a.name}`);
      if (p) {
        at[`app:${a.name}`] = p;
        boxes.push(p);
      }
    }
    let cell = 0;
    const clear = (c: { x: number; y: number }) => !boxes.some((b) => Math.abs(b.x - c.x) < APP_W + 16 && Math.abs(b.y - c.y) < APP_H + 16);
    for (const a of m.apps) {
      const id = `app:${a.name}`;
      if (at[id]) continue;
      let c: { x: number; y: number };
      do {
        c = { x: 420 + Math.floor(cell / 9) * 220, y: 40 + (cell % 9) * 80 };
        cell++;
      } while (!clear(c) && cell < 400);
      at[id] = c;
      boxes.push(c);
    }
    return at;
  };

  const node = (id: string, kind: string, cls: string): HTMLElement => {
    let el = els.get(id);
    if (!el) {
      const created = h('div', { class: 'network-node' });
      created.dataset.node = id;
      created.dataset.kind = kind;
      const port = h('span', { class: 'network-port', 'aria-hidden': 'true' });
      created.append(h('div', { class: 'network-node-body' }), port);
      port.addEventListener('pointerdown', (e) => startWire(e, id));
      // The ring of a circle or the edge of a box starts a wire, its middle moves it.
      created.addEventListener('pointerdown', (e) => {
        if (e.target instanceof Element && e.target.closest('.network-port, button, .network-grip')) return;
        if (onEdge(e, created)) startWire(e, id);
        else startMove(e, id);
      });
      created.addEventListener('pointermove', (e) => {
        if (!moving && !wiring) created.classList.toggle('edge', onEdge(e, created));
      });
      created.addEventListener('pointerleave', () => created.classList.remove('edge'));
      if (kind === 'app') {
        created.tabIndex = 0;
        // Enter only: Space is held to move the view.
        created.addEventListener('keydown', (e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            pick(id.slice(4));
          }
        });
      }
      layer.append(created);
      els.set(id, created);
      el = created;
    }
    el.className = `network-node ${kind} ${cls}`;
    return el;
  };

  function onEdge(e: PointerEvent, el: HTMLElement): boolean {
    const r = el.getBoundingClientRect();
    if (el.dataset.kind === 'app') {
      const d = Math.min(e.clientX - r.left, r.right - e.clientX, e.clientY - r.top, r.bottom - e.clientY);
      return d <= Math.min(RIM, r.height / 3);
    }
    const R = r.width / 2;
    const d = Math.hypot(e.clientX - (r.left + R), e.clientY - (r.top + R));
    // The Combined circle is large, so only a band along its line; a connection's whole outer ring.
    return d <= R + 2 && R - d <= (el.dataset.kind === 'merge' ? RIM + 4 : Math.max(RIM, R * 0.32));
  }
  const put = (el: HTMLElement, p: { x: number; y: number }, circle?: number) => {
    el.style.left = `${p.x}px`;
    el.style.top = `${p.y}px`;
    if (circle) {
      el.style.width = `${circle}px`;
      el.style.height = `${circle}px`;
    }
  };

  function sync() {
    if (moving || wiring) return;
    const n = N();
    const control = n.control && n.running;
    // The header stays one short line: the how-to is the title's tooltip, and only a paused state is said in it.
    title.title = control ? tr('network.drag') : `${tr('network.watching')} ${tr('network.drag')}`;
    note.textContent = n.paused ? tr('network.paused') : '';
    note.title = note.textContent;
    totals.textContent = `${tr('network.total')}: ${speed(n.total_down)} / ${speed(n.total_up)}`;
    const m = model();
    const at = layout(m);
    current = { m, at };
    const seen = new Set<string>();
    if (m.merge.length) {
      const el = node('merge', 'merge', '');
      const body = el.firstChild as HTMLElement;
      const split = button(tr('network.merge.split'), () => send1('network.merge', []), { small: true });
      body.replaceChildren(h('div', { class: 'network-node-title', text: tr('network.hub.merge') }), split);
      if (!el.querySelector('.network-grip')) {
        const grip = h('span', { class: 'network-grip', title: tr('network.merge.resize') });
        grip.addEventListener('pointerdown', (e) => startResize(e));
        el.append(grip);
      }
      put(el, at.merge, at.merge.size);
      seen.add('merge');
    }
    for (const l of m.links) {
      const id = `link:${l.name}`;
      const q = l.away ? 'unknown' : l.quality || 'unknown';
      const el = node(id, 'link', `${l.primary ? 'primary ' : ''}${l.away ? 'away ' : ''}${at[id].inside ? 'inside ' : ''}q-${q}`);
      const said = q === 'unknown' ? '' : tr(`network.q.${q}`, { ms: l.rtt, loss: Math.round((l.loss || 0) * 100) });
      el.title = [l.primary ? tr('network.link.primary') : l.away ? tr('network.link.away') : '', said].filter(Boolean).join('\n');
      (el.firstChild as HTMLElement).replaceChildren(
        iconEl(linkSymbol(l as { label: string }), 'network-link-glyph'),
        h('div', { class: 'network-node-title', text: l.label || l.name }),
        h('div', { class: 'network-node-sub', text: l.away ? tr('network.link.away') : l.name }),
        h('div', { class: 'network-node-speed', text: l.away ? '' : speed(l.down) + (l.rtt && !at[id].inside ? `, ${l.rtt} ms` : '') }),
      );
      put(el, at[id], at[id].d);
      seen.add(id);
    }
    for (const a of m.apps) {
      const id = `app:${a.name}`;
      const el = node(id, 'app', `${a.wire ? 'wired ' : ''}${a.idle ? 'idle ' : ''}${picked === a.name ? 'picked' : ''}`);
      el.title = a.idle ? tr('network.app.idle') : '';
      const wired = a.wire ? `  ${a.wire.target === 'block' ? tr('network.blocked') : tr(`network.priority.${a.wire.priority}`)}` : '';
      const refused = a.refused ? `  ${tr('network.refused', { n: a.refused })}` : '';
      (el.firstChild as HTMLElement).replaceChildren(
        // The glyph lies under the icon, so an app whose icon cannot be had still shows one.
        h('span', { class: 'network-app-icon' }, iconEl('app.dashed', 'network-app-glyph'), a.bundle ? iconImg(a.bundle) : null),
        h('div', { class: 'network-app-text' }, h('div', { class: 'network-node-title', text: a.name }), h('div', { class: 'network-node-sub', text: `${speed(a.down)} / ${speed(a.up)}${wired}${refused}` })),
      );
      put(el, at[id]);
      seen.add(id);
    }
    for (const [id, el] of els) {
      if (!seen.has(id)) {
        el.remove();
        els.delete(id);
      }
    }
    let w = 0;
    let hgt = 0;
    for (const [id, p] of Object.entries(at)) {
      w = Math.max(w, p.x + (p.size || (id.startsWith('app:') ? APP_W : D)) + 400);
      hgt = Math.max(hgt, p.y + (p.size || D) + 300);
    }
    board.style.width = `${Math.max(w, scroller.clientWidth || 900)}px`;
    board.style.height = `${Math.max(hgt, scroller.clientHeight || 600)}px`;
    wires();
    if (picked) fillCard(picked);
  }

  // Anchors: the right edge of a circle, the left edge of an app box.
  const out = (id: string): { x: number; y: number } | null => {
    const p = current!.at[id];
    if (!p) return null;
    if (id === 'merge') return { x: p.x + (p.size ?? 0), y: p.y + (p.size ?? 0) / 2 };
    const d = p.d || D;
    return { x: p.x + d, y: p.y + d / 2 };
  };
  const into = (id: string): { x: number; y: number } | null => {
    const p = current!.at[id];
    return p ? { x: p.x, y: p.y + APP_H / 2 } : null;
  };
  const thick = (b: number) => 1.5 + Math.min(6, Math.log10(1 + (Number(b) || 0) / 1e4) * 1.6);
  const line = (from: { x: number; y: number }, to: { x: number; y: number }, cls: string, width: number, curved: boolean): SVGPathElement => {
    const p = document.createElementNS(NS, 'path');
    const mx = (from.x + to.x) / 2;
    p.setAttribute('d', curved ? `M ${from.x} ${from.y} C ${mx} ${from.y}, ${mx} ${to.y}, ${to.x} ${to.y}` : `M ${from.x} ${from.y} L ${to.x} ${to.y}`);
    p.setAttribute('class', cls);
    p.style.strokeWidth = `${width}px`;
    svg.append(p);
    return p;
  };
  // What moves on each wire: its path, how fast down (connection to app) and up (app to connection), its colour.
  let flows: Flow[] = [];
  const reduced = !!window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  // The quality of where a wire comes from: a connection's own, the best of those in the Combined circle.
  const qOf = (id: string): string => {
    const links = current!.m.links;
    const of = (name: string) => {
      const l = links.find((x) => x.name === name);
      return l && !l.away && l.quality in RANK ? (l.quality as string) : null;
    };
    if (id === 'merge') {
      const best = current!.m.merge
        .map(of)
        .filter((q): q is string => !!q)
        .sort((a, b) => RANK[b] - RANK[a])[0];
      return best || 'unknown';
    }
    return of(id.slice(5)) || 'unknown';
  };
  const flowLine = (fromId: string, to: { x: number; y: number }, cls: string, width: number, a: Loose): SVGPathElement => {
    const q = qOf(fromId);
    const p = line(out(fromId)!, to, `${cls} q-${q}`, width, true);
    arrow(p, cls);
    // Nothing moves on a wire whose connection gets no answer.
    if ((a.down || 0) + (a.up || 0) > 500 && q !== 'down') flows.push({ path: p, down: a.down || 0, up: a.up || 0, width, cls, q, len: 0, dots: [] });
    return p;
  };
  // A small head at the app's end, so the way a wire goes reads even when nothing moves on it.
  function arrow(p: SVGPathElement, cls: string) {
    const len = p.getTotalLength ? p.getTotalLength() : 0;
    if (len < 24) return;
    const tip = p.getPointAtLength(len);
    const back = p.getPointAtLength(len - 9);
    const ang = Math.atan2(tip.y - back.y, tip.x - back.x);
    const s = 6;
    const pt = (r: number, k: number) => `${tip.x + Math.cos(ang + r) * k},${tip.y + Math.sin(ang + r) * k}`;
    const headEl = document.createElementNS(NS, 'polygon');
    headEl.setAttribute('points', [pt(0, 0), pt(Math.PI - 0.5, s * 1.6), pt(Math.PI + 0.5, s * 1.6)].join(' '));
    headEl.setAttribute('class', `network-arrow ${cls.replace('network-wire', '').trim()}`);
    svg.append(headEl);
  }
  function wires() {
    svg.replaceChildren();
    flows = [];
    const { m, at } = current!;
    const control = N().control && N().running;
    // The straight wire between the connections of the Combined circle.
    for (let i = 1; i < m.merge.length; i++) {
      // From edge to edge, along the line between the two middles, wherever they were put.
      const a = at[`link:${m.merge[i - 1]}`];
      const b = at[`link:${m.merge[i]}`];
      const ca = { x: a.x + C / 2, y: a.y + C / 2 };
      const cb = { x: b.x + C / 2, y: b.y + C / 2 };
      const d = Math.hypot(cb.x - ca.x, cb.y - ca.y) || 1;
      const ux = (cb.x - ca.x) / d;
      const uy = (cb.y - ca.y) / d;
      if (d > C) line({ x: ca.x + (ux * C) / 2, y: ca.y + (uy * C) / 2 }, { x: cb.x - (ux * C) / 2, y: cb.y - (uy * C) / 2 }, 'network-wire join', 3, false);
    }
    for (const a of m.apps) {
      const to = into(`app:${a.name}`);
      if (!to) continue;
      if (a.wire && a.wire.target !== 'system') {
        const fromId = a.wire.target === 'merge' ? (m.merge.length ? 'merge' : null) : a.wire.target;
        if (fromId && out(fromId)) flowLine(fromId, to, `network-wire ${control ? a.wire.priority : 'planned'}`, control ? thick(a.down) : 1.8, a);
        continue;
      }
      if (a.idle) continue;
      // Wired by itself: the connections its sockets were seen on, else the one macOS uses.
      const seen = (a.interfaces ?? []).map((x: Loose) => `link:${x.name}`).filter((id: string) => at[id]);
      const primary = m.links.find((l) => l.primary);
      const froms: string[] = seen.length ? seen : primary ? [`link:${primary.name}`] : [];
      for (const id of froms) flowLine(id, to, 'network-wire seen', thick(a.down), a);
    }
    // The dots: made here, moved by the clock in tick(), so a redraw every second does not make them jump.
    for (const f of flows) {
      f.len = f.path.getTotalLength();
      const make = (dir: number, rate: number) => {
        if (rate <= 500) return;
        const gap = dir > 0 ? 34 : 46;
        const count = Math.max(1, Math.floor(f.len / gap));
        // Faster with more traffic: from a slow walk to a run, on a log scale.
        const v = 28 + 34 * Math.log10(1 + rate / 2e3);
        for (let i = 0; i < count; i++) {
          const c = document.createElementNS(NS, 'circle');
          c.setAttribute('r', String(dir > 0 ? Math.max(2.2, f.width * 0.75) : 1.9));
          c.setAttribute('class', `network-flow ${dir > 0 ? 'down ' : 'up '}${f.cls.replace('network-wire', '').trim()}`);
          svg.append(c);
          f.dots.push({ c, dir, gap, v, i });
        }
      };
      make(1, f.down);
      make(-1, f.up);
    }
    tick();
    if (flows.length && !reduced && !ticking) {
      ticking = true;
      requestAnimationFrame(loop);
    }
  }
  let ticking = false;
  function loop() {
    if (!alive || !flows.length || !root.isConnected) {
      ticking = false;
      return;
    }
    tick();
    requestAnimationFrame(loop);
  }
  function tick() {
    const now = performance.now() / 1000;
    for (const f of flows) {
      for (const d of f.dots) {
        const span = Math.max(f.len, d.gap);
        // Strong: smooth. Fair: slower. Weak: it stalls and lurches, each dot on a beat of its own, and flickers.
        let time = now;
        if (f.q === 'fair') time = now * 0.7;
        else if (f.q === 'poor') time = now * 0.55 + 0.35 * Math.sin(now * 4.2 + d.i * 1.9) + 0.2 * Math.sin(now * 9.7 + d.i);
        // Evenly spaced along the wire, moving along it; down from the connection's end, up from the app's.
        let s = ((((reduced ? 0 : time * d.v) + d.i * d.gap) % span) + span) % span;
        if (d.dir < 0) s = span - s;
        const pt = f.path.getPointAtLength(Math.min(s, f.len));
        d.c.setAttribute('cx', String(pt.x));
        d.c.setAttribute('cy', String(pt.y));
        // Faded in at the start and out at the end, so dots do not pop.
        const edge = Math.min(s, f.len - s);
        let o = Math.max(0, Math.min(1, edge / 18));
        if (f.q === 'poor' && !reduced) o *= 0.35 + 0.65 * Math.abs(Math.sin(now * 7.3 + d.i * 2.3));
        d.c.style.opacity = String(o);
      }
    }
  }

  // Moving a node: it follows the pointer, its wires with it, and it lands on the grid.
  function startMove(e: PointerEvent, id: string) {
    if (e.button !== 0) return;
    e.preventDefault();
    const el = els.get(id)!;
    const cur = current!;
    const inside = !!cur.at[id] && !!cur.at[id].inside;
    const start = { x: e.clientX, y: e.clientY };
    const from = { x: cur.at[id].x, y: cur.at[id].y };
    let moved = false;
    moving = id;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const dx = (ev.clientX - start.x) / view.z;
      const dy = (ev.clientY - start.y) / view.z;
      if (!moved && Math.abs(dx) + Math.abs(dy) < 4 / view.z) return;
      moved = true;
      const M = cur.at.merge;
      if (inside) {
        // A connection in the Combined circle moves about inside it, never out of it.
        const rel = inCircle({ x: from.x + dx - M.x, y: from.y + dy - M.y }, M.size ?? MERGE_MIN);
        Object.assign(cur.at[id], { rel, x: M.x + rel.x, y: M.y + rel.y });
        put(el, cur.at[id], C);
        wires();
        return;
      }
      const p = { x: Math.max(0, from.x + dx), y: Math.max(0, from.y + dy) };
      cur.at[id].x = p.x;
      cur.at[id].y = p.y;
      if (id === 'merge') {
        for (const name of cur.m.merge) {
          const c = cur.at[`link:${name}`];
          c.x = p.x + (c.rel?.x ?? 0);
          c.y = p.y + (c.rel?.y ?? 0);
          put(els.get(`link:${name}`)!, c, C);
        }
      }
      put(el, p);
      wires();
    };
    const up = () => {
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
      moving = null;
      if (moved && inside) {
        const name = id.slice(5);
        const rel = cur.at[id].rel!;
        local.set(`in:${name}`, rel);
        send1('network.place', { id: `in:${name}`, x: rel.x, y: rel.y });
      } else if (moved) {
        const p = { x: snap(cur.at[id].x), y: snap(cur.at[id].y) };
        local.set(id, p);
        send1('network.place', { id, x: p.x, y: p.y });
      } else if (id.startsWith('app:')) {
        pick(id.slice(4));
      }
      sync();
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
  }

  // The grip of the Combined circle makes it larger or smaller; the connections in it stay inside.
  function startResize(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const grip = e.currentTarget as HTMLElement;
    const cur = current!;
    const M = cur.at.merge;
    const first = M.size ?? MERGE_MIN;
    const start = { x: e.clientX, y: e.clientY, size: first };
    moving = 'merge';
    grip.setPointerCapture(e.pointerId);
    const names = cur.m.merge;
    // Each connection keeps its place relative to the middle of the circle as it grows.
    const offsets = names.map((n) => {
      const r = cur.at[`link:${n}`].rel!;
      return { x: r.x + C / 2 - first / 2, y: r.y + C / 2 - first / 2 };
    });
    const move = (ev: PointerEvent) => {
      const grow = Math.max(ev.clientX - start.x, ev.clientY - start.y) / view.z;
      const circle = Math.round(Math.min(MERGE_MAX, Math.max(MERGE_MIN, start.size + grow)));
      M.size = circle;
      put(els.get('merge')!, M, circle);
      names.forEach((n, i) => {
        const rel = inCircle({ x: circle / 2 + offsets[i].x - C / 2, y: circle / 2 + offsets[i].y - C / 2 }, circle);
        Object.assign(cur.at[`link:${n}`], { rel, x: M.x + rel.x, y: M.y + rel.y });
        put(els.get(`link:${n}`)!, cur.at[`link:${n}`], C);
      });
      wires();
    };
    const up = () => {
      grip.removeEventListener('pointermove', move);
      grip.removeEventListener('pointerup', up);
      grip.removeEventListener('pointercancel', up);
      moving = null;
      const final = M.size ?? MERGE_MIN;
      local.set('merge:size', { x: final, y: final });
      send1('network.place', { id: 'merge:size', x: final, y: final });
      for (const n of names) {
        const rel = cur.at[`link:${n}`].rel!;
        local.set(`in:${n}`, rel);
        send1('network.place', { id: `in:${n}`, x: rel.x, y: rel.y });
      }
      sync();
    };
    grip.addEventListener('pointermove', move);
    grip.addEventListener('pointerup', up);
    grip.addEventListener('pointercancel', up);
  }

  // Wiring: from the dot of a node to another node.
  function startWire(e: PointerEvent, id: string) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const from = (id.startsWith('app:') ? into(id) : out(id))!;
    const state = { id, line: null as SVGPathElement | null };
    wiring = state;
    // The node under the pointer, else the nearest one within a little of it: a wire need not land on a point.
    const target = (ev: PointerEvent): HTMLElement | null => {
      const over = document.elementFromPoint(ev.clientX, ev.clientY);
      const el = over && over.closest ? (over.closest('[data-node]') as HTMLElement | null) : null;
      if (el && el.dataset.node !== id) return el;
      let best: HTMLElement | null = null;
      let bestD = 24;
      for (const [nid, n] of els) {
        if (nid === id) continue;
        const r = n.getBoundingClientRect();
        const d = Math.hypot(Math.max(r.left - ev.clientX, 0, ev.clientX - r.right), Math.max(r.top - ev.clientY, 0, ev.clientY - r.bottom));
        if (d < bestD) {
          best = n;
          bestD = d;
        }
      }
      return best;
    };
    const move = (ev: PointerEvent) => {
      if (state.line) state.line.remove();
      const b = board.getBoundingClientRect();
      state.line = line(from, { x: (ev.clientX - b.left) / view.z, y: (ev.clientY - b.top) / view.z }, 'network-wire dragging', 2.5, false);
      const over = target(ev);
      for (const el of els.values()) el.classList.toggle('drop', el === over);
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      const over = target(ev);
      wiring = null;
      for (const el of els.values()) el.classList.remove('drop');
      if (over && over.dataset.node) connect(id, over.dataset.node);
      sync();
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }

  // What a wire from one node to another means.
  function connect(a: string, b: string) {
    const kind = (id: string) => (id === 'merge' ? 'merge' : id.split(':')[0]);
    const linkOf = (id: string) => id.slice(5);
    const merge = (N().merge ?? []).slice();
    const place = (id: string) => (id === 'merge' ? 'merge' : `link:${linkOf(id)}`);
    if (kind(a) === 'app' && kind(b) !== 'app') return send1('network.wire', { app: a.slice(4), target: place(b) });
    if (kind(b) === 'app' && kind(a) !== 'app') return send1('network.wire', { app: b.slice(4), target: place(a) });
    if (kind(a) === 'app') return;
    // A connection wired to another, or to the circle: the Combined circle around them.
    const names = [a, b].filter((id) => id !== 'merge').map(linkOf);
    const joined: string[] = merge.length && (names.some((x) => merge.includes(x)) || a === 'merge' || b === 'merge') ? merge : [];
    for (const x of names) if (!joined.includes(x)) joined.push(x);
    if (joined.length >= 2) send1('network.merge', joined.slice(0, 4));
  }

  // Arrange: the connections down the left, the apps in columns to their right, in the order asked for; each box is put there
  // and kept, as if moved by hand.
  function arrange(by: string) {
    if (!current) return;
    const { m, at } = current;
    const rate = (a: Loose) => (a.down || 0) + (a.up || 0);
    const via = (a: Loose): string =>
      a.wire && a.wire.target !== 'system' ? a.wire.target : (a.interfaces ?? []).length ? `link:${a.interfaces[0].name}` : a.idle ? '~idle' : 'system';
    const places: Array<[string, number, number]> = [];
    let y = 40;
    if (m.merge.length) {
      places.push(['merge', 40, y]);
      y += (at.merge.size ?? MERGE_MIN) + 40;
    }
    const order: string[] = [];
    if (m.merge.length) order.push('merge');
    for (const l of m.links) {
      if (m.merge.includes(l.name)) {
        order.push(`link:${l.name}`);
        continue;
      }
      places.push([`link:${l.name}`, 60, y]);
      order.push(`link:${l.name}`);
      y += D + 50;
    }
    const left = 60 + Math.max(D, m.merge.length ? (at.merge.size ?? MERGE_MIN) - 20 : 0) + 140;
    const byName = (a: Loose, b: Loose) => a.name.localeCompare(b.name);
    const byRate = (a: Loose, b: Loose) => rate(b) - rate(a) || byName(a, b);
    let groups: Loose[][];
    if (by === 'link') {
      const rank = (k: string) => (k === '~idle' ? 1e3 : k === 'system' ? 999 : Math.max(0, order.indexOf(k)));
      const map = new Map<string, Loose[]>();
      for (const a of m.apps) {
        const k = via(a);
        if (!map.has(k)) map.set(k, []);
        map.get(k)!.push(a);
      }
      groups = [...map.entries()].sort((p, q) => rank(p[0]) - rank(q[0])).map(([, list]) => list.sort(byRate));
    } else {
      groups = [m.apps.slice().sort(by === 'name' ? byName : byRate)];
    }
    const ROWS = 9;
    let col = 0;
    for (const list of groups) {
      list.forEach((a, i) => {
        if (i && i % ROWS === 0) col++;
        places.push([`app:${a.name}`, left + col * 220, 40 + (i % ROWS) * 80]);
      });
      col++;
    }
    for (const [id, x, py] of places) {
      const p = { x: snap(x), y: snap(py) };
      local.set(id, p);
      send1('network.place', { id, x: p.x, y: p.y });
    }
    sync();
    fit();
  }

  // ---- the card of one app
  const cardTitle = h('h3');
  const held = h('p', { class: 'network-note' });
  const targetSel = h('select', { class: 'select', 'aria-label': tr('network.card.target') });
  const prioSel = h('select', { class: 'select', 'aria-label': tr('network.card.priority') });
  for (const p of ['high', 'normal', 'low']) prioSel.append(h('option', { value: p, text: tr(`network.priority.${p}`) }));
  const numberField = (max: number, step: number, label: string) =>
    h('input', { type: 'number', min: '0', max: String(max), step: String(step), class: 'text-field network-limit', placeholder: tr('network.card.limit.none'), 'aria-label': label });
  const downIn = numberField(1000000, 64, tr('network.card.down'));
  const upIn = numberField(1000000, 64, tr('network.card.up'));
  const quotaIn = numberField(10000000, 50, tr('network.card.quota'));
  const quotaPeriod = h('select', { class: 'select', 'aria-label': tr('network.card.quota.period') }, ...['day', 'month'].map((v) => h('option', { value: v, text: tr(`network.period.${v}`) })));
  const quotaUsed = h('p', { class: 'network-note' });
  const hostsTitle = h('div', { class: 'network-hosts-title', text: tr('network.card.hosts') });
  const hostsBox = h('div', { class: 'network-hosts' });
  const field = (label: string, control: HTMLElement) => h('label', { class: 'network-field' }, h('span', { text: label }), control);
  const limit = (input: HTMLInputElement) => {
    const v = Math.round(Number(input.value));
    return v > 0 ? Math.min(v, 1000000) : 0;
  };
  const save = button(
    tr('network.card.save'),
    () => {
      if (!picked) return;
      const quota = Math.round(Number(quotaIn.value));
      send1('network.rule', {
        app: picked,
        target: targetSel.value,
        priority: prioSel.value,
        down_kbps: limit(downIn),
        up_kbps: limit(upIn),
        quota_mb: quota > 0 ? Math.min(quota, 10000000) : 0,
        quota_period: quotaPeriod.value,
      });
    },
    { primary: true, small: true },
  );
  const unwire = button(tr('network.card.unwire'), () => {
    if (picked) send1('network.unwire', picked);
  }, { small: true });
  const shut = button(tr('network.close'), () => unpick(), { small: true });
  detail.append(
    cardTitle,
    held,
    h('div', { class: 'network-fields' }, field(tr('network.card.target'), targetSel), field(tr('network.card.priority'), prioSel), field(tr('network.card.down'), downIn), field(tr('network.card.up'), upIn), field(tr('network.card.quota'), quotaIn), field(tr('network.card.quota.period'), quotaPeriod)),
    quotaUsed,
    hostsTitle,
    hostsBox,
    h('div', { class: 'network-actions' }, shut, unwire, save),
  );
  function pick(app: string) {
    picked = app;
    detail.hidden = false;
    fillCard(app, true);
    for (const [id, el] of els) el.classList.toggle('picked', id === `app:${app}`);
  }
  function unpick() {
    picked = null;
    detail.hidden = true;
    for (const el of els.values()) el.classList.remove('picked');
  }
  function fillCard(app: string, fresh?: boolean) {
    const n = N();
    const wire = (n.wires ?? []).find((w) => w.app.toLowerCase() === app.toLowerCase());
    const a = (n.apps ?? []).find((x) => x.name === app);
    cardTitle.textContent = app;
    held.textContent = a && a.held_to ? tr('network.card.held', { speed: speed(a.held_to) }) : '';
    const blocked = wire && wire.target === 'block';
    if (blocked) held.textContent = tr('network.card.block.note');
    held.hidden = !held.textContent;
    const q = (n.policy?.quotas ?? []).find((x) => x.app.toLowerCase() === app.toLowerCase());
    quotaUsed.textContent = q ? tr('network.card.quota.used', { used: size(q.used), quota: size(q.quota) }) : '';
    quotaUsed.hidden = !quotaUsed.textContent;
    const hosts = (a && a.hosts) || [];
    hostsBox.replaceChildren(
      ...(hosts.length
        ? hosts.slice(0, 8).map((x) => h('div', { class: 'network-host' }, h('span', { class: 'network-host-name', text: x.name }), h('span', { class: 'network-host-size', text: size((x.received || 0) + (x.sent || 0)) })))
        : [h('div', { class: 'row-desc', text: tr('network.card.hosts.none') })]),
    );
    if (!fresh) return;
    const links = current ? current.m.links : [];
    const merge = n.merge ?? [];
    const options: Array<[string, string]> = [
      ['system', tr('network.hub.system')],
      ['block', tr('network.hub.block')],
      ['merge', merge.length >= 2 ? `${tr('network.hub.merge')} (${merge.map((x) => linkName(n, x)).join(' + ')})` : tr('network.hub.merge.all')],
      ...links.map((l): [string, string] => [`link:${l.name}`, `${l.label || l.name} (${l.name})`]),
    ];
    targetSel.replaceChildren(...options.map(([v, label]) => h('option', { value: v, text: label })));
    targetSel.value = wire ? wire.target : 'system';
    prioSel.value = wire ? wire.priority : 'normal';
    downIn.value = wire && wire.down_kbps ? String(wire.down_kbps) : '';
    upIn.value = wire && wire.up_kbps ? String(wire.up_kbps) : '';
    quotaIn.value = wire && wire.quota_mb ? String(wire.quota_mb) : '';
    quotaPeriod.value = wire ? wire.quota_period : 'month';
    unwire.hidden = !wire;
  }

  // The window listens to the whole of it: the keys, a click away, a resize.
  const onBlur = () => {
    spaceDown = false;
    scroller.classList.remove('space');
  };
  const onEscape = (e: KeyboardEvent) => {
    if (e.key !== 'Escape') return;
    if (!detail.hidden) unpick();
    else send({ t: 'action', name: 'network_canvas_close' });
  };
  const onResize = () => sync();
  window.addEventListener('keydown', onKey);
  window.addEventListener('keyup', onKey);
  window.addEventListener('blur', onBlur);
  window.addEventListener('keydown', onEscape);
  window.addEventListener('resize', onResize);
  stops.push(() => {
    window.removeEventListener('keydown', onKey);
    window.removeEventListener('keyup', onKey);
    window.removeEventListener('blur', onBlur);
    window.removeEventListener('keydown', onEscape);
    window.removeEventListener('resize', onResize);
  });

  applyView();
  sync();
  // The diagram follows the state of the app, and takes the icons of apps as they arrive.
  stops.push(
    useApp.subscribe((s, prev) => {
      if (s.state !== prev.state || s.lang !== prev.lang) sync();
      if (s.icons !== prev.icons) refreshIcons(root);
    }),
  );

  return () => {
    alive = false;
    for (const stop of stops) stop();
    host.replaceChildren();
  };
}
