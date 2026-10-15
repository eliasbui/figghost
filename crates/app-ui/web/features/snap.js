// Dragging a window onto the notch to snap it to halves and thirds.
// There is no page of its own: a card at the end of "Window snapping" holds the switch, the Accessibility state
// and the choice of zones, since the layouts are the same ones the shortcuts above apply.
(function () {
  const OFFERED = ['left', 'right', 'first_third', 'center_third', 'last_third', 'maximize', 'top', 'bottom', 'top_left', 'top_right', 'bottom_left', 'bottom_right'];
  const DEFAULT_ZONES = OFFERED.slice(0, 6);
  const MAX = 8;

  const glyph = (key) => {
    const [x, y, w, hh] = LAYOUT_RECTS[key];
    return h('div', { class: 'mini', 'aria-hidden': 'true' }, h('i', { style: `left:${x * 100}%;top:${y * 100}%;width:${w * 100}%;height:${hh * 100}%` }));
  };

  // The row as it will hang from the notch: a dark shape with one cell per chosen zone.
  function preview() {
    const row = h('div', { class: 'snap-row' });
    const shape = h('div', { class: 'snap-shape', role: 'img' }, row);
    reg(() => {
      shape.setAttribute('aria-label', t('snap.preview') + ': ' + S.snap.zones.map((k) => t('layout.' + k)).join(', '));
      row.replaceChildren(...S.snap.zones.map((k) => h('span', { class: 'snap-cell' }, glyph(k))));
    });
    return h('div', { class: 'snap-stage' }, shape);
  }

  function pick(key) {
    const el = h('button', { class: 'snap-pick', type: 'button', 'aria-pressed': 'false' }, glyph(key), h('span', { class: 'snap-name', text: t('layout.' + key) }));
    el.addEventListener('click', () => {
      const chosen = new Set(S.snap.zones);
      if (chosen.has(key)) chosen.delete(key); else chosen.add(key);
      if (chosen.size < 1 || chosen.size > MAX) return;
      change('snap.zones', OFFERED.filter((k) => chosen.has(k)));
    });
    reg(() => {
      const on = S.snap.zones.includes(key);
      el.setAttribute('aria-pressed', String(on));
      // The last zone cannot be removed and a ninth cannot be added; say so instead of ignoring the click.
      el.disabled = on ? S.snap.zones.length <= 1 : S.snap.zones.length >= MAX;
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
      'snap.enable.desc': 'Drag a window by its title bar to the top edge of the screen. A row of zones appears under the notch; release over one to place the window there. Releasing anywhere else changes nothing.',
      'snap.access.title': 'Accessibility', 'snap.access.desc': 'Placing a window needs Accessibility access. Without it nothing happens when a window is dropped.',
      'snap.access.ok': 'Granted', 'snap.access.no': 'Not granted', 'snap.access.open': 'Open System Settings',
      'snap.zones.title': 'Zones', 'snap.zones.desc': 'Choose which places appear under the notch.',
      'snap.zones.count': '{n} of {max} chosen', 'snap.zones.reset': 'Restore the default six', 'snap.preview': 'The zones under the notch',
      'fault.snap_zones': 'Choose between 1 and 8 zones.', 'fault.snap_failed': 'Could not watch the mouse for window drags.',
    },
    vi: {
      'sec.snap': 'Kéo lên notch',
      'snap.enable.title': 'Ghim cửa sổ bằng cách kéo lên notch',
      'snap.enable.desc': 'Kéo cửa sổ bằng thanh tiêu đề lên mép trên màn hình. Một hàng vùng ghim hiện ra dưới notch; thả chuột trên một vùng để đặt cửa sổ vào đó. Thả ở chỗ khác thì không có gì thay đổi.',
      'snap.access.title': 'Quyền Trợ năng', 'snap.access.desc': 'Đặt cửa sổ cần quyền Trợ năng (Accessibility). Khi chưa cấp quyền, thả cửa sổ sẽ không có tác dụng.',
      'snap.access.ok': 'Đã cấp', 'snap.access.no': 'Chưa cấp', 'snap.access.open': 'Mở Cài đặt Hệ thống',
      'snap.zones.title': 'Các vùng', 'snap.zones.desc': 'Chọn những vị trí hiện ra dưới notch.',
      'snap.zones.count': 'Đã chọn {n} trên {max}', 'snap.zones.reset': 'Khôi phục sáu vùng mặc định', 'snap.preview': 'Các vùng dưới notch',
      'fault.snap_zones': 'Hãy chọn từ 1 đến 8 vùng.', 'fault.snap_failed': 'Không theo dõi được chuột để nhận biết việc kéo cửa sổ.',
    },
    slots: { tiling: cards },
    selftest({ expect, go, sent, S: state }) {
      go('tiling');
      const picks = [...document.querySelectorAll('.snap-pick')];
      expect(picks.length === OFFERED.length, 'one choice per offered layout');
      expect(picks.filter((p) => p.getAttribute('aria-pressed') === 'true').length === state.snap.zones.length, 'the chosen zones are marked');
      expect(document.querySelectorAll('.snap-cell').length === state.snap.zones.length, 'the preview has a cell per zone');
      sent.length = 0;
      const top = picks[OFFERED.indexOf('top')];
      top.click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'snap.zones' && m.value.includes('top') && m.value.length === 7), 'choosing a zone sends the list: ' + JSON.stringify(sent));
      expect(state.snap.zones.indexOf('top') > state.snap.zones.indexOf('maximize'), 'in the order the row shows them');
      // The last zone cannot be removed.
      state.snap.zones = ['left'];
      state.snap.enabled = true;
      runSyncs();
      const left = [...document.querySelectorAll('.snap-pick')][0];
      expect(left.disabled, 'the last zone cannot be unchecked');
      // A ninth cannot be added.
      state.snap.zones = OFFERED.slice(0, MAX);
      runSyncs();
      const extra = [...document.querySelectorAll('.snap-pick')][MAX];
      expect(extra.disabled, 'a ninth zone cannot be added');
      // A second change of the same setting within 40 ms is sent a moment later, so the local state is what shows.
      [...document.querySelectorAll('.snap-foot .btn')][0].click();
      expect(state.snap.zones.join() === OFFERED.slice(0, 6).join(), 'the reset button restores the six');
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
