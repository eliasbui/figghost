// Dragging a window onto the notch to snap it into a layout.
// There is no page of its own: cards at the end of "Window snapping" hold the switch, the Accessibility state, the
// choice of layouts for each screen and the layouts a person makes from a grid.
(function () {
  // core-window's snap::PER_ROW, templates::MAX_GRID, MAX_CUSTOM and MAX_NAME, and templates::DEFAULT.
  const PER_ROW = 10;
  const MAX_GRID = 6;
  const MAX_CUSTOM = 12;
  const MAX_NAME = 24;
  const DEFAULT = ['full', 'halves', 'thirds', 'quarters', 'rows', 'main_left', 'main_right'];

  // The screen the list below is for ('' for all the screens that have none of their own), and the layout being made
  // or changed. They outlive the cards, which are built again each time the page is opened.
  let screen = '';
  let editing = null;

  // ---------------------------------------------------------------- what the state holds

  const customs = () => S.snap.custom;
  /** Every layout that can be chosen, in the order of the choices: the built-in ones, then the person's. */
  const catalog = () => [...S.snap.built_in.map((b) => b.id), ...customs().map((c) => c.id)];
  /** The cells of a layout as fractions of the screen: x0, y0, x1, y1. */
  function cellsOf(id) {
    const built = S.snap.built_in.find((b) => b.id === id);
    if (built) return built.cells;
    const mine = customs().find((c) => c.id === id);
    return mine ? mine.cells.map(([c0, r0, c1, r1]) => [c0 / mine.cols, r0 / mine.rows, c1 / mine.cols, r1 / mine.rows]) : [];
  }
  const nameOf = (id) => {
    const mine = customs().find((c) => c.id === id);
    return mine ? mine.name : t('snap.t.' + id);
  };
  /** The ids chosen for the screen being looked at. */
  const chosen = () => (screen && S.snap.screens[screen] ? S.snap.screens[screen] : S.snap.templates);
  /** The same list for screens that have none of their own: the screens are named in `S.snap.displays`. */
  const screens = () => [...new Set([...S.snap.displays, ...Object.keys(S.snap.screens)])];

  function write(list) {
    if (!screen) change('snap.templates', list);
    else change('snap.screens', { ...S.snap.screens, [screen]: list });
  }

  // ---------------------------------------------------------------- drawing a layout

  /** A small screen with the cells of a layout. */
  function mini(cells, cls) {
    const kids = cells.map(([x0, y0, x1, y1]) => h('i', {
      style: `left:calc(${x0 * 100}% + .75px);top:calc(${y0 * 100}% + .75px);width:calc(${(x1 - x0) * 100}% - 1.5px);height:calc(${(y1 - y0) * 100}% - 1.5px)`,
    }));
    return h('div', { class: 'snap-mini' + (cls ? ' ' + cls : ''), 'aria-hidden': 'true' }, kids);
  }

  /** How many layouts go on each row of the row under the notch: as even as can be, none more than PER_ROW. */
  function rowLengths(count) {
    if (count < 1) return [];
    const rows = Math.ceil(count / PER_ROW);
    const base = Math.floor(count / rows), extra = count % rows;
    return Array.from({ length: rows }, (_, i) => base + (i < extra ? 1 : 0));
  }

  // The layouts as they will hang from the notch: a dark shape with the layouts in rows, each a small screen with
  // its name.
  function preview() {
    const shape = h('div', { class: 'snap-shape', role: 'img' });
    reg(() => {
      const ids = chosen();
      shape.setAttribute('aria-label', t('snap.preview') + ': ' + ids.map(nameOf).join(', '));
      let at = 0;
      shape.replaceChildren(...rowLengths(ids.length).map((n) => {
        const keys = ids.slice(at, at += n);
        return h('div', { class: 'snap-row' },
          keys.map((id) => h('span', { class: 'snap-cell' }, mini(cellsOf(id)), h('span', { class: 'snap-cap', text: nameOf(id) }))));
      }));
    });
    return h('div', { class: 'snap-stage' }, shape);
  }

  // ---------------------------------------------------------------- choosing the layouts of a screen

  /** The choices, one button for each layout. A layout made or deleted adds or takes away a button; the others stay. */
  function pickList() {
    const el = h('div', { class: 'snap-picks', role: 'group', 'aria-label': t('snap.layouts.title') });
    const buttons = new Map();
    const make = (id) => {
      const b = h('button', { class: 'snap-pick', type: 'button', 'aria-pressed': 'false' }, h('span', { class: 'snap-pick-mini' }), h('span', { class: 'snap-name' }));
      b.addEventListener('click', () => {
        const on = chosen().includes(id);
        const next = catalog().filter((k) => (k === id ? !on : chosen().includes(k)));
        if (next.length < 1) return;
        write(next);
      });
      return b;
    };
    reg(() => {
      const ids = catalog();
      for (const id of [...buttons.keys()]) if (!ids.includes(id)) { buttons.get(id).remove(); buttons.delete(id); }
      ids.forEach((id, i) => {
        let b = buttons.get(id);
        if (!b) { b = make(id); buttons.set(id, b); }
        if (el.children[i] !== b) el.insertBefore(b, el.children[i] || null);
        const on = chosen().includes(id);
        b.setAttribute('aria-pressed', String(on));
        // The last layout cannot be removed; say so instead of ignoring the click.
        b.disabled = on && chosen().length <= 1;
        // The drawing and the name change when a layout is edited.
        const look = JSON.stringify(cellsOf(id)) + nameOf(id);
        if (b.dataset.look !== look) {
          b.dataset.look = look;
          b.firstElementChild.replaceChildren(mini(cellsOf(id)));
          b.lastElementChild.textContent = nameOf(id);
        }
      });
    });
    return el;
  }

  /** The pop-up that says which screen the list is for. */
  function screenPicker() {
    const el = h('select', { class: 'pick', 'aria-label': t('snap.screen.title') });
    el.addEventListener('change', () => { screen = el.value; runSyncs(); });
    reg(() => {
      if (screen && !screens().includes(screen)) screen = '';
      const options = [{ value: '', label: t('snap.screen.all') }, ...screens().map((name) => ({
        value: name,
        label: S.snap.displays.includes(name) ? name : t('snap.screen.away', { name }),
      }))];
      const key = JSON.stringify(options) + lang;
      if (el.dataset.key !== key) {
        el.dataset.key = key;
        el.replaceChildren(...options.map((o) => h('option', { value: o.value, text: o.label })));
      }
      el.value = screen;
    });
    return el;
  }

  function access() {
    const state = pill(() => (S.snap.accessibility ? t('snap.access.ok') : t('snap.access.no')), () => S.snap.accessibility);
    const open = h('button', { class: 'btn small', onclick: () => send({ t: 'action', name: 'open_accessibility' }) }, t('snap.access.open'));
    reg(() => { open.style.display = S.snap.accessibility ? 'none' : ''; });
    return row(t('snap.access.title'), t('snap.access.desc'), [state, open]);
  }

  function layoutsCard() {
    const count = h('div', { class: 'row-desc', role: 'status' });
    const note = h('div', { class: 'row-desc' });
    const follow = button(t('snap.layouts.follow'), () => {
      const rest = { ...S.snap.screens };
      delete rest[screen];
      change('snap.screens', rest);
    }, { icon: 'refresh', small: true });
    const defaults = button(t('snap.layouts.reset'), () => write(DEFAULT.slice()), { icon: 'refresh', small: true });
    reg(() => {
      count.textContent = t('snap.layouts.count', { n: chosen().length });
      note.textContent = !screen ? t('snap.screen.note.all') : S.snap.screens[screen] ? t('snap.screen.note.own') : t('snap.screen.note.shared');
      follow.style.display = screen && S.snap.screens[screen] ? '' : 'none';
    });
    const body = h('div', { class: 'snap-body' },
      preview(),
      pickList(),
      h('div', { class: 'snap-foot' }, h('div', {}, count, note), h('div', { class: 'snap-actions' }, follow, defaults)));
    const el = card(
      row(t('snap.screen.title'), t('snap.layouts.desc'), screenPicker()),
      body);
    reg(() => el.classList.toggle('dim', !S.snap.enabled));
    return el;
  }

  // ---------------------------------------------------------------- layouts made from a grid

  /** The parts of a grid of `cols` by `rows`, each as [column, row, column after, row after]. */
  const base = (cols, rows) => Array.from({ length: cols * rows }, (_, i) => [i % cols, Math.floor(i / cols), (i % cols) + 1, Math.floor(i / cols) + 1]);
  const inReadingOrder = (regions) => regions.slice().sort((a, b) => a[1] - b[1] || a[0] - b[0]);

  /**
   * Joins the picked regions into one: the smallest rectangle that holds them, grown until it cuts no other region
   * in two. The others inside it go; the rest stay.
   */
  function join(regions, picked) {
    let [c0, r0, c1, r1] = [Infinity, Infinity, 0, 0];
    for (const i of picked) {
      const g = regions[i];
      c0 = Math.min(c0, g[0]); r0 = Math.min(r0, g[1]); c1 = Math.max(c1, g[2]); r1 = Math.max(r1, g[3]);
    }
    for (let grew = true; grew;) {
      grew = false;
      for (const g of regions) {
        const touches = g[0] < c1 && g[2] > c0 && g[1] < r1 && g[3] > r0;
        if (touches && (g[0] < c0 || g[1] < r0 || g[2] > c1 || g[3] > r1)) {
          c0 = Math.min(c0, g[0]); r0 = Math.min(r0, g[1]); c1 = Math.max(c1, g[2]); r1 = Math.max(r1, g[3]);
          grew = true;
        }
      }
    }
    const inside = (g) => g[0] >= c0 && g[1] >= r0 && g[2] <= c1 && g[3] <= r1;
    return inReadingOrder([...regions.filter((g) => !inside(g)), [c0, r0, c1, r1]]);
  }

  /** Takes one joined region apart into the parts of the grid it covers. */
  function split(regions, i) {
    const [c0, r0, c1, r1] = regions[i];
    const parts = base(c1 - c0, r1 - r0).map(([a, b, c, d]) => [a + c0, b + r0, c + c0, d + r0]);
    return inReadingOrder([...regions.filter((_, k) => k !== i), ...parts]);
  }

  const nextId = () => {
    for (let n = 1; n < 10000; n += 1) if (!customs().some((c) => c.id === 'c' + n)) return 'c' + n;
    return 'c1';
  };

  function startEditing(layout) {
    editing = layout
      ? { id: layout.id, name: layout.name, cols: layout.cols, rows: layout.rows, regions: inReadingOrder(layout.cells.map((c) => c.slice())), picked: [] }
      : { id: '', name: '', cols: 2, rows: 1, regions: base(2, 1), picked: [] };
  }

  function save(host, problem) {
    const name = editing.name.trim();
    if (!name) { problem.textContent = t('snap.edit.name.needed'); return; }
    const layout = { id: editing.id || nextId(), name, cols: editing.cols, rows: editing.rows, cells: inReadingOrder(editing.regions) };
    const fresh = !editing.id;
    const list = fresh ? [...customs(), layout] : customs().map((c) => (c.id === layout.id ? layout : c));
    change('snap.custom', list);
    // A layout just made is offered where the list being looked at is, so that it shows at the next drag.
    if (fresh) write(catalog().filter((k) => k === layout.id || chosen().includes(k)));
    editing = null;
    redraw(host);
  }

  function remove(id, host) {
    confirmDialog({ title: t('snap.delete.title'), body: t('snap.delete.body'), confirm: t('snap.mine.delete') }, () => {
      change('snap.custom', customs().filter((c) => c.id !== id));
      // The app takes it off every list too; the page follows without waiting for the answer.
      const keep = (ids) => ids.filter((k) => k !== id);
      S.snap.templates = keep(S.snap.templates).length ? keep(S.snap.templates) : DEFAULT.slice();
      for (const [name, ids] of Object.entries(S.snap.screens)) {
        if (keep(ids).length) S.snap.screens[name] = keep(ids); else delete S.snap.screens[name];
      }
      redraw(host);
    });
  }

  function editor(host) {
    const problem = h('div', { class: 'snap-problem', role: 'alert' });
    const name = h('input', { class: 'text-field', type: 'text', maxlength: String(MAX_NAME), value: editing.name, placeholder: t('snap.edit.name.ph'), 'aria-label': t('snap.edit.name'), spellcheck: 'false', autocomplete: 'off' });
    name.addEventListener('input', () => { editing.name = name.value; problem.textContent = ''; });
    const size = (key, label) => {
      const el = h('select', { class: 'pick small', 'aria-label': label },
        Array.from({ length: MAX_GRID }, (_, i) => h('option', { value: String(i + 1), text: String(i + 1) })));
      el.value = String(editing[key]);
      el.addEventListener('change', () => {
        editing[key] = +el.value;
        editing.regions = base(editing.cols, editing.rows);
        editing.picked = [];
        redraw(host);
      });
      return h('label', { class: 'snap-size' }, h('span', { text: label }), el);
    };
    const grid = h('div', { class: 'snap-grid', style: `grid-template-columns:repeat(${editing.cols},1fr);grid-template-rows:repeat(${editing.rows},1fr)` },
      editing.regions.map((g, i) => h('button', {
        class: 'snap-region', type: 'button', 'aria-pressed': String(editing.picked.includes(i)),
        'aria-label': t('snap.edit.cell', { n: i + 1 }),
        style: `grid-column:${g[0] + 1}/${g[2] + 1};grid-row:${g[1] + 1}/${g[3] + 1}`,
        text: String(i + 1),
        onclick: () => {
          editing.picked = editing.picked.includes(i) ? editing.picked.filter((k) => k !== i) : [...editing.picked, i];
          redraw(host);
        },
      })));
    const joinable = editing.picked.length >= 2;
    const splittable = editing.picked.length === 1 && (() => { const g = editing.regions[editing.picked[0]]; return g[2] - g[0] > 1 || g[3] - g[1] > 1; })();
    const joinBtn = button(t('snap.edit.join'), () => { editing.regions = join(editing.regions, editing.picked); editing.picked = []; redraw(host); }, { small: true });
    const splitBtn = button(t('snap.edit.split'), () => { editing.regions = split(editing.regions, editing.picked[0]); editing.picked = []; redraw(host); }, { small: true });
    joinBtn.disabled = !joinable;
    splitBtn.disabled = !splittable;
    return h('div', { class: 'snap-editor' },
      h('div', { class: 'snap-edit-head' }, h('div', { class: 'row-title', text: editing.id ? t('snap.edit.title.change') : t('snap.edit.title.new') })),
      h('div', { class: 'snap-edit-fields' }, name, size('cols', t('snap.edit.cols')), size('rows', t('snap.edit.rows'))),
      grid,
      h('div', { class: 'snap-edit-tools' }, joinBtn, splitBtn),
      h('div', { class: 'row-desc', text: t('snap.edit.hint') }),
      problem,
      h('div', { class: 'snap-edit-foot' },
        button(t('snap.edit.cancel'), () => { editing = null; redraw(host); }),
        button(t('snap.edit.save'), () => save(host, problem), { primary: true })));
  }

  function mineRow(layout, host) {
    return h('div', { class: 'snap-mine' },
      mini(cellsOf(layout.id), 'big'),
      h('div', { class: 'snap-mine-name' }, h('div', { class: 'row-title', text: layout.name }), h('div', { class: 'row-desc', text: t('snap.mine.cells', { n: layout.cells.length }) })),
      h('div', { class: 'snap-actions' },
        button(t('snap.mine.edit'), () => { startEditing(layout); redraw(host); }, { small: true }),
        h('button', { class: 'btn icon', 'aria-label': t('snap.mine.delete') + ': ' + layout.name, title: t('snap.mine.delete'), onclick: () => remove(layout.id, host) }, icon('trash'))));
  }

  /** Draws the card of the layouts a person made into `host`: the list, or the editor in its place. */
  function redraw(host) {
    if (editing) { host.replaceChildren(editor(host)); return; }
    const add = button(t('snap.mine.new'), () => { startEditing(null); redraw(host); }, { icon: 'plus', small: true });
    add.disabled = customs().length >= MAX_CUSTOM;
    host.replaceChildren(
      customs().length ? h('div', { class: 'snap-mines' }, customs().map((c) => mineRow(c, host))) : h('div', { class: 'row-desc', text: t('snap.mine.none') }),
      h('div', { class: 'snap-foot' }, h('div', { class: 'row-desc', text: t('snap.mine.count', { n: customs().length, max: MAX_CUSTOM }) }), add));
  }

  function mineCard() {
    const host = h('div', { class: 'snap-body' });
    redraw(host);
    return card(row(t('snap.mine.title'), t('snap.mine.desc'), null), host);
  }

  function cards() {
    const first = card(toggleRow('snap.enabled', t('snap.enable.title'), t('snap.enable.desc')), access());
    return [sectionTitle(t('sec.snap')), first, layoutsCard(), mineCard()];
  }

  (window.__plugins = window.__plugins || []).push({
    id: 'snap',
    en: {
      'sec.snap': 'Drag to the notch',
      'snap.enable.title': 'Snap by dragging to the notch',
      'snap.enable.desc': 'Drag a window by its title bar to the top edge of the screen. The layouts appear under the notch. Point at one to see its cells on the screen, then move down into a cell and release to put the window there. Releasing anywhere else changes nothing.',
      'snap.access.title': 'Accessibility', 'snap.access.desc': 'Placing a window needs Accessibility access. Without it nothing happens when a window is dropped.',
      'snap.access.ok': 'Granted', 'snap.access.no': 'Not granted', 'snap.access.open': 'Open System Settings',
      'snap.screen.title': 'Layouts of a screen', 'snap.layouts.title': 'Layouts',
      'snap.layouts.desc': 'Choose which layouts appear under the notch. Every screen can have a list of its own, and the others follow the list of all screens.',
      'snap.screen.all': 'All screens', 'snap.screen.away': '{name} (not connected)',
      'snap.screen.note.all': 'These layouts appear on every screen that has no list of its own.',
      'snap.screen.note.own': 'This screen has a list of its own.',
      'snap.screen.note.shared': 'This screen follows the list of all screens until you change a layout here.',
      'snap.layouts.count': '{n} chosen', 'snap.layouts.reset': 'Restore the defaults', 'snap.layouts.follow': 'Follow all screens',
      'snap.preview': 'The layouts under the notch',
      'snap.t.full': 'Full screen', 'snap.t.halves': 'Halves', 'snap.t.rows': 'Rows', 'snap.t.thirds': 'Thirds', 'snap.t.quarters': 'Quarters',
      'snap.t.main_left': 'Wide left', 'snap.t.main_right': 'Wide right', 'snap.t.left_stack': '1 + 2', 'snap.t.right_stack': '2 + 1', 'snap.t.grid_six': 'Six',
      'snap.mine.title': 'Your layouts', 'snap.mine.desc': 'Divide the screen into a grid and join parts of it into larger cells. A layout you make can be chosen for any screen above.',
      'snap.mine.none': 'No layouts of your own yet.', 'snap.mine.new': 'New layout', 'snap.mine.edit': 'Edit', 'snap.mine.delete': 'Delete',
      'snap.mine.count': '{n} of {max}', 'snap.mine.cells': '{n} cells',
      'snap.edit.title.new': 'New layout', 'snap.edit.title.change': 'Change layout',
      'snap.edit.name': 'Name', 'snap.edit.name.ph': 'Name of the layout', 'snap.edit.name.needed': 'Type a name for the layout.',
      'snap.edit.cols': 'Columns', 'snap.edit.rows': 'Rows', 'snap.edit.cell': 'Cell {n}',
      'snap.edit.join': 'Join', 'snap.edit.split': 'Split',
      'snap.edit.hint': 'Select two or more cells and press Join to make one larger cell. Select a joined cell and press Split to take it apart. Changing the columns or rows starts the grid again.',
      'snap.edit.save': 'Save', 'snap.edit.cancel': 'Cancel',
      'snap.delete.title': 'Delete this layout?', 'snap.delete.body': 'It is taken off the screens that offered it.',
      'fault.snap_templates': 'Choose at least one layout.', 'fault.snap_custom': 'That layout cannot be saved.',
      'fault.snap_failed': 'Could not watch the mouse for window drags.',
    },
    vi: {
      'sec.snap': 'Kéo lên notch',
      'snap.enable.title': 'Ghim cửa sổ bằng cách kéo lên notch',
      'snap.enable.desc': 'Kéo cửa sổ bằng thanh tiêu đề lên mép trên màn hình. Các bố cục hiện ra dưới notch. Chỉ vào một bố cục để xem các ô của nó trên màn hình, rồi đưa xuống một ô và thả chuột để đặt cửa sổ vào đó. Thả ở chỗ khác thì không có gì thay đổi.',
      'snap.access.title': 'Quyền Trợ năng', 'snap.access.desc': 'Đặt cửa sổ cần quyền Trợ năng (Accessibility). Khi chưa cấp quyền, thả cửa sổ sẽ không có tác dụng.',
      'snap.access.ok': 'Đã cấp', 'snap.access.no': 'Chưa cấp', 'snap.access.open': 'Mở Cài đặt Hệ thống',
      'snap.screen.title': 'Bố cục của màn hình', 'snap.layouts.title': 'Bố cục',
      'snap.layouts.desc': 'Chọn những bố cục hiện ra dưới notch. Mỗi màn hình có thể có danh sách riêng, các màn hình còn lại theo danh sách chung của mọi màn hình.',
      'snap.screen.all': 'Mọi màn hình', 'snap.screen.away': '{name} (đang không kết nối)',
      'snap.screen.note.all': 'Các bố cục này hiện trên mọi màn hình chưa có danh sách riêng.',
      'snap.screen.note.own': 'Màn hình này có danh sách riêng.',
      'snap.screen.note.shared': 'Màn hình này đang theo danh sách chung cho đến khi bạn đổi một bố cục ở đây.',
      'snap.layouts.count': 'Đã chọn {n}', 'snap.layouts.reset': 'Khôi phục mặc định', 'snap.layouts.follow': 'Theo mọi màn hình',
      'snap.preview': 'Các bố cục dưới notch',
      'snap.t.full': 'Tối đa', 'snap.t.halves': 'Chia đôi', 'snap.t.rows': 'Hai hàng', 'snap.t.thirds': 'Chia ba', 'snap.t.quarters': 'Bốn góc',
      'snap.t.main_left': 'Rộng trái', 'snap.t.main_right': 'Rộng phải', 'snap.t.left_stack': '1 + 2', 'snap.t.right_stack': '2 + 1', 'snap.t.grid_six': 'Sáu ô',
      'snap.mine.title': 'Bố cục của bạn', 'snap.mine.desc': 'Chia màn hình thành lưới rồi gộp các phần thành ô lớn hơn. Bố cục bạn tạo có thể chọn cho bất kỳ màn hình nào ở trên.',
      'snap.mine.none': 'Chưa có bố cục riêng nào.', 'snap.mine.new': 'Bố cục mới', 'snap.mine.edit': 'Sửa', 'snap.mine.delete': 'Xóa',
      'snap.mine.count': '{n} trên {max}', 'snap.mine.cells': '{n} ô',
      'snap.edit.title.new': 'Bố cục mới', 'snap.edit.title.change': 'Sửa bố cục',
      'snap.edit.name': 'Tên', 'snap.edit.name.ph': 'Tên của bố cục', 'snap.edit.name.needed': 'Hãy nhập tên cho bố cục.',
      'snap.edit.cols': 'Cột', 'snap.edit.rows': 'Hàng', 'snap.edit.cell': 'Ô {n}',
      'snap.edit.join': 'Gộp', 'snap.edit.split': 'Tách',
      'snap.edit.hint': 'Chọn hai ô trở lên rồi bấm Gộp để thành một ô lớn hơn. Chọn một ô đã gộp rồi bấm Tách để tách ra lại. Đổi số cột hoặc hàng sẽ bắt đầu lại lưới.',
      'snap.edit.save': 'Lưu', 'snap.edit.cancel': 'Hủy',
      'snap.delete.title': 'Xóa bố cục này?', 'snap.delete.body': 'Bố cục sẽ bị gỡ khỏi các màn hình đang dùng nó.',
      'fault.snap_templates': 'Hãy chọn ít nhất một bố cục.', 'fault.snap_custom': 'Không lưu được bố cục này.',
      'fault.snap_failed': 'Không theo dõi được chuột để nhận biết việc kéo cửa sổ.',
    },
    slots: { tiling: cards },
    selftest({ expect, go, sent, S: state }) {
      go('tiling');
      editing = null;
      screen = '';
      const picks = () => [...document.querySelectorAll('.snap-pick')];
      const named = (label) => [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === label);
      const nameFor = (id) => t('snap.t.' + id);
      const count = state.snap.built_in.length + state.snap.custom.length;
      expect(picks().length === count, 'one choice per layout: ' + picks().length);
      expect(picks().filter((p) => p.getAttribute('aria-pressed') === 'true').length === state.snap.templates.length, 'the chosen layouts are marked');
      expect(document.querySelectorAll('.snap-cell').length === state.snap.templates.length, 'the preview has a cell per layout');
      expect(document.querySelectorAll('.snap-shape .snap-row').length === 1, 'seven layouts make one row');
      expect(document.querySelector('.snap-cap').textContent === nameFor(state.snap.templates[0]), 'each layout has its name');
      expect(document.querySelectorAll('.snap-cell')[3].querySelectorAll('.snap-mini i').length === 4, 'quarters are drawn with four cells');
      for (const [n, rows] of [[1, [1]], [7, [7]], [8, [8]], [9, [9]], [10, [10]], [11, [6, 5]], [15, [8, 7]], [22, [8, 7, 7]]]) {
        expect(rowLengths(n).join() === rows.join(), 'rows of ' + n + ': ' + rowLengths(n));
      }

      // Choosing a layout sends the whole list, in the order of the catalogue.
      sent.length = 0;
      const grid = picks()[catalog().indexOf('left_stack')];
      grid.click();
      const message = sent.find((m) => m.t === 'set' && m.path === 'snap.templates');
      expect(message && message.value.includes('left_stack') && message.value.length === state.snap.templates.length, 'choosing a layout sends the list: ' + JSON.stringify(sent));
      expect(state.snap.templates.indexOf('left_stack') === state.snap.templates.indexOf('main_right') + 1, 'in the order the layouts show');

      // The last layout cannot be removed, and every layout can be chosen at once.
      state.snap.templates = ['full'];
      runSyncs();
      expect(picks()[0].disabled, 'the last layout cannot be unchecked');
      state.snap.templates = catalog();
      runSyncs();
      expect(picks().every((p) => !p.disabled), 'all of them can be chosen');
      expect(document.querySelectorAll('.snap-shape .snap-row').length === 1, 'and fit on one row');
      named(t('snap.layouts.reset')).click();
      expect(state.snap.templates.join() === DEFAULT.join(), 'the reset button restores the defaults');

      // A screen with a list of its own: the choice goes to its own path and the others keep theirs.
      state.snap.displays = ['Built-in', 'DELL'];
      runSyncs();
      const select = document.querySelector('select[aria-label="' + t('snap.screen.title') + '"]');
      expect([...select.options].map((o) => o.value).join() === ',Built-in,DELL', 'the screens are offered: ' + [...select.options].map((o) => o.value));
      select.value = 'DELL';
      select.dispatchEvent(new Event('change'));
      expect(picks().filter((p) => p.getAttribute('aria-pressed') === 'true').length === DEFAULT.length, 'a screen without a list shows the common one');
      expect(named(t('snap.layouts.follow')).style.display === 'none', 'and has nothing to follow');
      sent.length = 0;
      picks()[catalog().indexOf('grid_six')].click();
      const own = sent.find((m) => m.t === 'set' && m.path === 'snap.screens');
      expect(own && own.value.DELL && own.value.DELL.includes('grid_six') && !sent.some((m) => m.path === 'snap.templates'), 'its own list is sent: ' + JSON.stringify(sent));
      expect(state.snap.templates.join() === DEFAULT.join(), 'the list of all screens is untouched');
      expect(named(t('snap.layouts.follow')).style.display !== 'none', 'it can go back to following');
      named(t('snap.layouts.follow')).click();
      // A second change of the same setting within 40 ms is sent a moment later, so the local state is what shows.
      expect(!state.snap.screens.DELL, 'following all screens drops its list');
      select.value = '';
      select.dispatchEvent(new Event('change'));

      // A layout made from a grid: three columns by two rows, the first two parts of the first row joined.
      expect(document.querySelector('.snap-mine') === null && document.body.textContent.includes(t('snap.mine.none')), 'no layout of the person yet');
      named(t('snap.mine.new')).click();
      expect(document.querySelector('.snap-editor') && document.querySelectorAll('.snap-region').length === 2, 'the editor starts with two parts');
      const field = () => document.querySelector('.snap-editor input.text-field');
      const size = (label, value) => {
        const el = document.querySelector('.snap-editor select[aria-label="' + label + '"]');
        el.value = String(value);
        el.dispatchEvent(new Event('change'));
      };
      size(t('snap.edit.cols'), 3);
      size(t('snap.edit.rows'), 2);
      expect(document.querySelectorAll('.snap-region').length === 6, 'a grid of three by two has six parts');
      expect(named(t('snap.edit.join')).disabled && named(t('snap.edit.split')).disabled, 'nothing to join or split yet');
      const regions = () => [...document.querySelectorAll('.snap-region')];
      regions()[0].click();
      regions()[1].click();
      expect(!named(t('snap.edit.join')).disabled, 'two parts can be joined');
      named(t('snap.edit.join')).click();
      expect(regions().length === 5 && regions()[0].style.gridColumn === '1 / 3', 'joined: five cells, the first two parts wide');
      regions()[0].click();
      expect(!named(t('snap.edit.split')).disabled, 'a joined cell can be split');
      named(t('snap.edit.split')).click();
      expect(regions().length === 6, 'split: six again');
      regions()[0].click();
      regions()[1].click();
      named(t('snap.edit.join')).click();
      // Saving without a name asks for one and sends nothing.
      sent.length = 0;
      named(t('snap.edit.save')).click();
      expect(document.querySelector('.snap-problem').textContent === t('snap.edit.name.needed') && sent.length === 0, 'a name is needed');
      field().value = 'Coding';
      field().dispatchEvent(new Event('input'));
      named(t('snap.edit.save')).click();
      const saved = sent.find((m) => m.t === 'set' && m.path === 'snap.custom');
      expect(saved && saved.value.length === 1 && saved.value[0].id === 'c1' && saved.value[0].name === 'Coding', 'the layout is sent: ' + JSON.stringify(sent));
      expect(saved && JSON.stringify(saved.value[0].cells) === JSON.stringify([[0, 0, 2, 1], [2, 0, 3, 1], [0, 1, 1, 2], [1, 1, 2, 2], [2, 1, 3, 2]]), 'with cells that cover the grid once: ' + JSON.stringify(saved && saved.value[0].cells));
      expect(state.snap.templates.includes('c1'), 'and it is offered at once');
      expect(document.querySelector('.snap-editor') === null && document.querySelector('.snap-mine'), 'the list is back, with the layout in it');
      expect(picks().length === count + 1, 'and it can be chosen');

      // Joining takes in whatever it would cut in two.
      const grown = join([[0, 0, 2, 1], [2, 0, 3, 1], [0, 1, 1, 2], [1, 1, 3, 2]], [0, 2]);
      expect(JSON.stringify(grown) === '[[0,0,3,2]]', 'a join takes in every cell it would cut in two: ' + JSON.stringify(grown));
      expect(JSON.stringify(split([[0, 0, 2, 1], [2, 0, 3, 1]], 0)) === JSON.stringify([[0, 0, 1, 1], [1, 0, 2, 1], [2, 0, 3, 1]]), 'a split gives back the parts');

      // Deleting asks, then drops the layout and takes it off the lists.
      document.querySelector('.snap-mine .btn.icon').click();
      document.querySelector('#modal .btn.primary').click();
      expect(state.snap.custom.length === 0 && !state.snap.templates.includes('c1'), 'a deleted layout goes from the list too');

      const sw = [...document.querySelectorAll('.switch')].find((s) => s.getAttribute('aria-label') === t('snap.enable.title'));
      expect(!!sw, 'the switch exists');
      sent.length = 0;
      sw.click();
      expect(sent.some((m) => m.path === 'snap.enabled'), 'the switch sends its setting');
      state.snap.accessibility = false;
      runSyncs();
      const open = [...document.querySelectorAll('button')].find((b) => b.textContent === t('snap.access.open'));
      expect(open && open.style.display !== 'none', 'a missing permission offers to open System Settings');
      sent.length = 0;
      open.click();
      expect(sent.some((m) => m.t === 'action' && m.name === 'open_accessibility'), 'and does');
      state.snap.accessibility = true;
      runSyncs();
    },
  });
})();
