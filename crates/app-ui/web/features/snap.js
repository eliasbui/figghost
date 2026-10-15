// Dragging a window onto the notch to snap it to halves, thirds, quarters or the whole screen.
// There is no page of its own: a card at the end of "Window snapping" holds the switch, the Accessibility state
// and the choice of zones, since the layouts are the same ones the shortcuts above apply.
(function () {
  // The order of core-window's snap::OFFERED: what takes the whole height of the screen goes on the first row,
  // what takes half of it on the second.
  const TALL = ['left', 'right', 'maximize', 'first_third', 'center_third', 'last_third'];
  const OFFERED = [...TALL, 'top', 'bottom', 'top_left', 'top_right', 'bottom_left', 'bottom_right'];
  const DEFAULT_ZONES = OFFERED.filter((k) => k !== 'top' && k !== 'bottom');
  // snap::MAX_ZONES: every offered layout fits.
  const MAX = OFFERED.length;

  const glyph = (key) => {
    const [x, y, w, hh] = LAYOUT_RECTS[key];
    return h('div', { class: 'mini', 'aria-hidden': 'true' }, h('i', { style: `left:${x * 100}%;top:${y * 100}%;width:${w * 100}%;height:${hh * 100}%` }));
  };

  // The zones as they will hang from the notch: a dark shape with a row for each kind of layout chosen, each zone
  // a small screen with its short name.
  function preview() {
    const shape = h('div', { class: 'snap-shape', role: 'img' });
    reg(() => {
      shape.setAttribute('aria-label', t('snap.preview') + ': ' + S.snap.zones.map((k) => t('layout.' + k)).join(', '));
      const rows = [S.snap.zones.filter((k) => TALL.includes(k)), S.snap.zones.filter((k) => !TALL.includes(k))].filter((r) => r.length);
      shape.replaceChildren(...rows.map((keys) => h('div', { class: 'snap-row' },
        keys.map((k) => h('span', { class: 'snap-cell' }, glyph(k), h('span', { class: 'snap-cap', text: t('snap.short.' + k) }))))));
    });
    return h('div', { class: 'snap-stage' }, shape);
  }

  function pick(key) {
    const el = h('button', { class: 'snap-pick', type: 'button', 'aria-pressed': 'false' }, glyph(key), h('span', { class: 'snap-name', text: t('layout.' + key) }));
    el.addEventListener('click', () => {
      const chosen = new Set(S.snap.zones);
      if (chosen.has(key)) chosen.delete(key); else chosen.add(key);
      if (chosen.size < 1) return;
      change('snap.zones', OFFERED.filter((k) => chosen.has(k)));
    });
    reg(() => {
      const on = S.snap.zones.includes(key);
      el.setAttribute('aria-pressed', String(on));
      // The last zone cannot be removed; say so instead of ignoring the click.
      el.disabled = on && S.snap.zones.length <= 1;
    });
    return el;
  }

  function access() {
    const state = pill(() => (S.snap.accessibility ? t('snap.access.ok') : t('snap.access.no')), () => S.snap.accessibility);
    const open = h('button', { class: 'btn small', onclick: () => send({ t: 'action', name: 'open_accessibility' }) }, t('snap.access.open'));
    reg(() => { open.style.display = S.snap.accessibility ? 'none' : ''; });
    return row(t('snap.access.title'), t('snap.access.desc'), [state, open]);
  }

  function cards() {
    const count = h('div', { class: 'row-desc', role: 'status' });
    reg(() => { count.textContent = t('snap.zones.count', { n: S.snap.zones.length, max: MAX }); });
    const zones = card(
      row(t('snap.zones.title'), t('snap.zones.desc'), null, { block: true }),
      h('div', { class: 'snap-body' },
        preview(),
        h('div', { class: 'snap-picks', role: 'group', 'aria-label': t('snap.zones.title') }, OFFERED.map(pick)),
        h('div', { class: 'snap-foot' }, count, button(t('snap.zones.reset'), () => change('snap.zones', DEFAULT_ZONES.slice()), { icon: 'refresh', small: true }))));
    reg(() => zones.classList.toggle('dim', !S.snap.enabled));
    return [
      sectionTitle(t('sec.snap')),
      card(toggleRow('snap.enabled', t('snap.enable.title'), t('snap.enable.desc')), access()),
      zones,
    ];
  }

  (window.__plugins = window.__plugins || []).push({
    id: 'snap',
    en: {
      'sec.snap': 'Drag to the notch',
      'snap.enable.title': 'Snap by dragging to the notch',
      'snap.enable.desc': 'Drag a window by its title bar to the top edge of the screen. The layouts appear under the notch; hold the pointer over one to see where the window will go, and release to place it there. Releasing anywhere else changes nothing.',
      'snap.access.title': 'Accessibility', 'snap.access.desc': 'Placing a window needs Accessibility access. Without it nothing happens when a window is dropped.',
      'snap.access.ok': 'Granted', 'snap.access.no': 'Not granted', 'snap.access.open': 'Open System Settings',
      'snap.zones.title': 'Zones', 'snap.zones.desc': 'Choose which layouts appear under the notch. Those that take the whole height of the screen go on the first row, the others on the second.',
      'snap.zones.count': '{n} of {max} chosen', 'snap.zones.reset': 'Restore the default ten', 'snap.preview': 'The zones under the notch',
      'snap.short.left': 'Left', 'snap.short.right': 'Right', 'snap.short.maximize': 'Full', 'snap.short.first_third': 'Left third', 'snap.short.center_third': 'Center third', 'snap.short.last_third': 'Right third',
      'snap.short.top': 'Top', 'snap.short.bottom': 'Bottom', 'snap.short.top_left': 'Top left', 'snap.short.top_right': 'Top right', 'snap.short.bottom_left': 'Bottom left', 'snap.short.bottom_right': 'Bottom right',
      'fault.snap_zones': 'Choose between 1 and 12 zones.', 'fault.snap_failed': 'Could not watch the mouse for window drags.',
    },
    vi: {
      'sec.snap': 'Kéo lên notch',
      'snap.enable.title': 'Ghim cửa sổ bằng cách kéo lên notch',
      'snap.enable.desc': 'Kéo cửa sổ bằng thanh tiêu đề lên mép trên màn hình. Các bố cục hiện ra dưới notch; giữ con trỏ trên một ô để xem cửa sổ sẽ nằm ở đâu, rồi thả chuột để đặt cửa sổ vào đó. Thả ở chỗ khác thì không có gì thay đổi.',
      'snap.access.title': 'Quyền Trợ năng', 'snap.access.desc': 'Đặt cửa sổ cần quyền Trợ năng (Accessibility). Khi chưa cấp quyền, thả cửa sổ sẽ không có tác dụng.',
      'snap.access.ok': 'Đã cấp', 'snap.access.no': 'Chưa cấp', 'snap.access.open': 'Mở Cài đặt Hệ thống',
      'snap.zones.title': 'Các vùng', 'snap.zones.desc': 'Chọn những bố cục hiện ra dưới notch. Bố cục cao hết màn hình nằm ở hàng đầu, các bố cục còn lại ở hàng thứ hai.',
      'snap.zones.count': 'Đã chọn {n} trên {max}', 'snap.zones.reset': 'Khôi phục mười vùng mặc định', 'snap.preview': 'Các vùng dưới notch',
      'snap.short.left': 'Trái', 'snap.short.right': 'Phải', 'snap.short.maximize': 'Tối đa', 'snap.short.first_third': '1/3 trái', 'snap.short.center_third': '1/3 giữa', 'snap.short.last_third': '1/3 phải',
      'snap.short.top': 'Trên', 'snap.short.bottom': 'Dưới', 'snap.short.top_left': 'Trên trái', 'snap.short.top_right': 'Trên phải', 'snap.short.bottom_left': 'Dưới trái', 'snap.short.bottom_right': 'Dưới phải',
      'fault.snap_zones': 'Hãy chọn từ 1 đến 12 vùng.', 'fault.snap_failed': 'Không theo dõi được chuột để nhận biết việc kéo cửa sổ.',
    },
    slots: { tiling: cards },
    selftest({ expect, go, sent, S: state }) {
      go('tiling');
      const picks = [...document.querySelectorAll('.snap-pick')];
      expect(picks.length === OFFERED.length, 'one choice per offered layout');
      expect(picks.filter((p) => p.getAttribute('aria-pressed') === 'true').length === state.snap.zones.length, 'the chosen zones are marked');
      expect(document.querySelectorAll('.snap-cell').length === state.snap.zones.length, 'the preview has a cell per zone');
      const rows = [...document.querySelectorAll('.snap-shape .snap-row')];
      expect(rows.length === 2 && rows[0].children.length === 6 && rows[1].children.length === 4, 'halves, full and thirds on one row, quarters on the next');
      expect(rows[0].querySelector('.snap-cap').textContent === t('snap.short.left'), 'each zone has its short name');
      sent.length = 0;
      const top = picks[OFFERED.indexOf('top')];
      top.click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'snap.zones' && m.value.includes('top') && m.value.length === state.snap.zones.length), 'choosing a zone sends the list: ' + JSON.stringify(sent));
      expect(state.snap.zones.indexOf('top') === state.snap.zones.indexOf('last_third') + 1, 'in the order the zones show');
      expect(document.querySelectorAll('.snap-shape .snap-row')[1].firstElementChild.querySelector('.snap-cap').textContent === t('snap.short.top'), 'on the second row');
      // The last zone cannot be removed.
      state.snap.zones = ['left'];
      state.snap.enabled = true;
      runSyncs();
      const left = [...document.querySelectorAll('.snap-pick')][0];
      expect(left.disabled, 'the last zone cannot be unchecked');
      // Every offered layout can be chosen at once.
      state.snap.zones = OFFERED.slice();
      runSyncs();
      expect([...document.querySelectorAll('.snap-pick')].every((p) => !p.disabled), 'all twelve can be chosen');
      expect(document.querySelectorAll('.snap-shape .snap-row').length === 2, 'and still make two rows');
      // A second change of the same setting within 40 ms is sent a moment later, so the local state is what shows.
      [...document.querySelectorAll('.snap-foot .btn')][0].click();
      expect(state.snap.zones.join() === DEFAULT_ZONES.join(), 'the reset button restores the ten');
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
