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
  },
  vi: {
    'nav.updates': 'Cập nhật', 'page.updates': 'Cập nhật',
    'page.updates.sub': 'Xem phần mềm nào trên máy có bản mới và cập nhật ngay trên notch.',
    'updates.enable.title': 'Hiện tab Cập nhật', 'updates.enable.desc': 'Formula và cask của Homebrew, gói npm toàn cục, ứng dụng App Store (cần công cụ mas), bản cập nhật của Apple, và các agent lập trình tự cập nhật: Claude Code, OpenCode, Pi, Droid và Amp. Phiên bản hiện tại màu đỏ, bản mới màu xanh; tích các dòng để cập nhật, hoặc cập nhật tất cả. Tự kiểm tra lại mỗi sáu giờ.',
    'updates.daily.title': 'Cập nhật tất cả mỗi ngày', 'updates.daily.desc': 'Mỗi ngày một lần, mọi thứ trừ bản cập nhật của Apple được cập nhật mà không hỏi. Bản cập nhật của Apple cần quyền quản trị nên sẽ mở Cài đặt hệ thống khi bạn tự bấm cập nhật.',
    'updates.check': 'Kiểm tra ngay', 'updates.checking': 'Đang kiểm tra...',
    'updates.count': 'Tìm thấy {n} bản cập nhật.',
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
    return [
      card(
        toggleRow('updates.enabled', t('updates.enable.title'), t('updates.enable.desc')),
        toggleRow('updates.daily', t('updates.daily.title'), t('updates.daily.desc'), { when: on }),
        row(t('updates.check'), null, [count, check], { when: on })),
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
  },
});
