// Updates: a tab of the notch with the software that has a newer version, from Homebrew, npm, the App Store, Apple
// and the coding agents, ticked and updated from the notch.
(window.__plugins = window.__plugins || []).push({
  id: 'updates',
  icon: 'arrow.down.circle',
  en: {
    'nav.updates': 'Updates', 'page.updates': 'Updates',
    'page.updates.sub': 'See what has a newer version on this Mac and update it from the notch.',
    'updates.enable.title': 'Show the Updates tab', 'updates.enable.desc': 'Homebrew formulae and casks, global npm packages, App Store apps (with the mas tool), Apple software updates, and the coding agents that update themselves: Claude Code, OpenCode, Pi, Droid and Amp. The version now is red and the new one green; tick rows to update them, or update all. Looks again every six hours.',
    'updates.daily.title': 'Update everything every day', 'updates.daily.desc': 'Once a day, everything except Apple\'s updates is updated without asking. Apple\'s updates need an administrator, so they open System Settings when you update them yourself.',
    'updates.check': 'Check now', 'updates.checking': 'Checking...',
    'updates.count': '{n} updates found.',
    'sec.updates.list': 'What can be updated', 'updates.empty': 'Everything is up to date.', 'updates.off': 'Turn on the tab to look for updates.', 'updates.pick.all': 'Choose all', 'updates.run': 'Update {n}', 'updates.all': 'Update all', 'updates.busy': 'Working...',
  },
  vi: {
    'nav.updates': 'Cập nhật', 'page.updates': 'Cập nhật',
    'page.updates.sub': 'Xem phần mềm nào trên máy có bản mới và cập nhật ngay trên notch.',
    'updates.enable.title': 'Hiện tab Cập nhật', 'updates.enable.desc': 'Formula và cask của Homebrew, gói npm toàn cục, ứng dụng App Store (cần công cụ mas), bản cập nhật của Apple, và các agent lập trình tự cập nhật: Claude Code, OpenCode, Pi, Droid và Amp. Phiên bản hiện tại màu đỏ, bản mới màu xanh; tích các dòng để cập nhật, hoặc cập nhật tất cả. Tự kiểm tra lại mỗi sáu giờ.',
    'updates.daily.title': 'Cập nhật tất cả mỗi ngày', 'updates.daily.desc': 'Mỗi ngày một lần, mọi thứ trừ bản cập nhật của Apple được cập nhật mà không hỏi. Bản cập nhật của Apple cần quyền quản trị nên sẽ mở Cài đặt hệ thống khi bạn tự bấm cập nhật.',
    'updates.check': 'Kiểm tra ngay', 'updates.checking': 'Đang kiểm tra...',
    'updates.count': 'Tìm thấy {n} bản cập nhật.',
    'sec.updates.list': 'Có thể cập nhật', 'updates.empty': 'Mọi thứ đã là bản mới nhất.', 'updates.off': 'Bật tab để tìm bản cập nhật.', 'updates.pick.all': 'Chọn tất cả', 'updates.run': 'Cập nhật {n}', 'updates.all': 'Cập nhật tất cả', 'updates.busy': 'Đang chạy...',
  },
  build() {
    const on = () => S.updates.enabled;
    const check = button(t('updates.check'), () => send({ t: 'action', name: 'updates_check' }), { small: true, icon: 'refresh' });
    const count = h('div', { class: 'row-desc' });
    reg(() => {
      check.lastChild.textContent = S.updates.busy ? t('updates.checking') : t('updates.check');
      check.disabled = S.updates.busy || !on();
      count.textContent = t('updates.count', { n: S.updates.count });
    });
    const chosen = new Set();
    const all = h('input', { type: 'checkbox', 'aria-label': t('updates.pick.all') });
    const run = button('', () => send({ t: 'action', name: 'updates_run:' + [...chosen].join('\n') }), { small: true, primary: true });
    const every = button(t('updates.all'), () => send({ t: 'action', name: 'updates_all' }), { small: true, icon: 'arrow.down.circle' });
    const box = h('div', { class: 'upd-list' });
    const items = () => S.updates.list || [];
    const paint = () => {
      const list = items();
      for (const k of [...chosen]) if (!list.some((u) => u.key === k)) chosen.delete(k);
      all.checked = list.length > 0 && chosen.size === list.length;
      all.indeterminate = chosen.size > 0 && chosen.size < list.length;
      all.disabled = !list.length || S.updates.busy;
      run.textContent = S.updates.busy ? t('updates.busy') : t('updates.run', { n: chosen.size });
      run.disabled = !chosen.size || S.updates.busy;
      every.disabled = !list.length || S.updates.busy;
    };
    const draw = () => {
      const list = items();
      if (!list.length) { box.replaceChildren(h('div', { class: 'empty', text: t(on() ? 'updates.empty' : 'updates.off') })); return; }
      box.replaceChildren(...list.map((u) => {
        const pick = h('input', { type: 'checkbox', 'aria-label': u.name });
        pick.checked = chosen.has(u.key);
        pick.addEventListener('change', () => { if (pick.checked) chosen.add(u.key); else chosen.delete(u.key); paint(); });
        return h('label', { class: 'upd-item' }, pick,
          h('span', { class: 'upd-text' }, h('span', { class: 'upd-name', text: u.name }), h('span', { class: 'upd-source', text: u.source })),
          h('span', { class: 'upd-versions' }, u.current && h('span', { class: 'upd-old', text: u.current }), u.current && h('span', { class: 'upd-arrow', text: '->' }), h('span', { class: 'upd-new', text: u.latest })));
      }));
    };
    all.addEventListener('change', () => { for (const u of items()) { if (all.checked) chosen.add(u.key); else chosen.delete(u.key); } draw(); paint(); });
    let last = null;
    reg(() => {
      const stamp = JSON.stringify([S.updates.list, on(), lang]);
      if (stamp !== last) { last = stamp; draw(); }
      paint();
    });
    return [
      card(
        toggleRow('updates.enabled', t('updates.enable.title'), t('updates.enable.desc')),
        toggleRow('updates.daily', t('updates.daily.title'), t('updates.daily.desc'), { when: on }),
        row(t('updates.check'), null, [count, check], { when: on })),
      sectionTitle(t('sec.updates.list')),
      card(h('div', { class: 'upd-toolbar' }, h('label', { class: 'upd-all' }, all, t('updates.pick.all')), run, every), box),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('updates');
    const switches = [...document.querySelectorAll('#scroller .switch')];
    expect(switches.length === 2, 'updates: a switch for the tab and one for the daily update');
    sent.length = 0;
    switches[1].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'updates.daily'), 'updates: the daily switch sends its setting: ' + JSON.stringify(sent));
    switches[1].click();
    sent.length = 0;
    const btn = [...document.querySelectorAll('#scroller .btn')].find((b) => b.textContent === t('updates.check'));
    btn.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'updates_check'), 'updates: check now asks the app');
    expect(document.body.textContent.includes(t('updates.count', { n: S.updates.count })), 'updates: the count is shown');
    const rows = [...document.querySelectorAll('.upd-item')];
    expect(rows.length === S.updates.list.length && rows[0].querySelector('.upd-old').textContent === '2.0.9' && rows[0].querySelector('.upd-new').textContent === '2.0.10', 'updates: each row shows the version now and the new one');
    rows[1].querySelector('input').click();
    rows[2].querySelector('input').click();
    sent.length = 0;
    [...document.querySelectorAll('#scroller .btn')].find((b) => b.textContent === t('updates.run', { n: 2 })).click();
    expect(sent.some((m) => m.name === 'updates_run:Npm:pnpm\nAgent:droid'), 'updates: the ticked rows are updated together: ' + JSON.stringify(sent));
    sent.length = 0;
    [...document.querySelectorAll('#scroller .btn')].find((b) => b.textContent === t('updates.all')).click();
    expect(sent.some((m) => m.name === 'updates_all'), 'updates: update all asks the app');
  },
});
