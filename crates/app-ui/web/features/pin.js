// Always on top: a shortcut keeps the window in front above the others, with a red border round it.
(window.__plugins = window.__plugins || []).push({
  id: 'pin',
  icon: 'pin.fill',
  en: {
    'nav.pin': 'Always on top', 'page.pin': 'Always on top',
    'page.pin.sub': 'Press the shortcut and the window in front stays above all the others, with a red border round it. Press it again to let go.',
    'target.pin': 'the Always on top shortcut',
    'pin.enable.title': 'Use Always on top', 'pin.enable.desc': 'One window is kept on top at a time, from the shortcut or from the switch in the menu bar panel. macOS lets a program raise only its own windows, so while another window covers the pinned one, Figghost shows a live picture of it over the cover. Click the picture and the real window comes forward.',
    'pin.shortcut.title': 'Shortcut', 'pin.shortcut.desc': 'Keeps the window in front on top, or lets go of the one that is. Works in any app. Press Delete while recording to remove it.',
    'pin.now.title': 'Window on top', 'pin.now.none': 'None', 'pin.now.release': 'Let go',
    'sec.pin.access': 'Permissions', 'pin.allowed': 'Allowed', 'pin.not_allowed': 'Not allowed',
    'pin.access.title': 'Accessibility', 'pin.access.desc': 'Finds the window in front, and brings the real window forward when you click its picture.',
    'pin.access.open': 'Open settings',
    'pin.screen.title': 'Screen Recording', 'pin.screen.desc': 'Takes the picture of the pinned window while it is covered. Pictures stay in memory; nothing is saved or sent. After you allow it, macOS may ask to quit and reopen Figghost.',
    'pin.screen.allow': 'Allow',
  },
  vi: {
    'nav.pin': 'Luôn trên cùng', 'page.pin': 'Luôn trên cùng',
    'page.pin.sub': 'Nhấn phím tắt để cửa sổ đang ở phía trước luôn nằm trên mọi cửa sổ khác, có viền đỏ bao quanh. Nhấn lần nữa để thả ra.',
    'target.pin': 'phím tắt Luôn trên cùng',
    'pin.enable.title': 'Dùng Luôn trên cùng', 'pin.enable.desc': 'Mỗi lần giữ một cửa sổ trên cùng, bằng phím tắt hoặc công tắc trong bảng ở thanh menu. macOS chỉ cho một chương trình nâng cửa sổ của chính nó, nên khi cửa sổ khác che cửa sổ đã ghim, Figghost hiện ảnh trực tiếp của nó lên trên phần che. Bấm vào ảnh thì cửa sổ thật được đưa lên trước.',
    'pin.shortcut.title': 'Phím tắt', 'pin.shortcut.desc': 'Giữ cửa sổ đang ở phía trước trên cùng, hoặc thả cửa sổ đang được giữ. Dùng ở mọi ứng dụng. Nhấn Delete khi đang ghi phím để xóa.',
    'pin.now.title': 'Cửa sổ trên cùng', 'pin.now.none': 'Không có', 'pin.now.release': 'Thả ra',
    'sec.pin.access': 'Quyền truy cập', 'pin.allowed': 'Đã cho phép', 'pin.not_allowed': 'Chưa cho phép',
    'pin.access.title': 'Trợ năng', 'pin.access.desc': 'Tìm cửa sổ đang ở phía trước, và đưa cửa sổ thật lên trước khi bạn bấm vào ảnh của nó.',
    'pin.access.open': 'Mở cài đặt',
    'pin.screen.title': 'Ghi màn hình', 'pin.screen.desc': 'Chụp ảnh cửa sổ đã ghim khi nó bị che. Ảnh chỉ nằm trong bộ nhớ; không có gì được lưu hay gửi đi. Sau khi bạn cho phép, macOS có thể yêu cầu thoát và mở lại Figghost.',
    'pin.screen.allow': 'Cho phép',
  },
  build() {
    const now = h('span', { class: 'row-title pin-now' });
    reg(() => { now.textContent = S.pin.pinned || t('pin.now.none'); now.title = S.pin.pinned || ''; });
    const release = button(t('pin.now.release'), () => send({ t: 'action', name: 'pin_release' }), { small: true });
    reg(() => { release.style.visibility = S.pin.pinned ? 'visible' : 'hidden'; });
    const when = { when: () => S.pin.enabled };
    return [
      card(toggleRow('pin.enabled', t('pin.enable.title'), t('pin.enable.desc'))),
      card(
        row(t('pin.shortcut.title'), t('pin.shortcut.desc'), hotkeyButton('pin', { allowClear: true, label: t('pin.shortcut.title') }), when),
        row(t('pin.now.title'), null, [now, release], when)),
      sectionTitle(t('sec.pin.access')),
      card(
        row(t('pin.access.title'), t('pin.access.desc'), [
          pill(() => (S.pin.accessibility ? t('pin.allowed') : t('pin.not_allowed')), () => S.pin.accessibility),
          button(t('pin.access.open'), () => send({ t: 'action', name: 'open_accessibility' }), { small: true }),
        ]),
        row(t('pin.screen.title'), t('pin.screen.desc'), [
          pill(() => (S.pin.screen_access ? t('pin.allowed') : t('pin.not_allowed')), () => S.pin.screen_access),
          button(t('pin.screen.allow'), () => send({ t: 'action', name: 'pin_screen_access' }), { small: true }),
        ])),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('pin');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const power = page.querySelector('.switch');
    const was = S.pin.enabled;
    power.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'pin.enabled' && m.value === !was), 'pin: the switch sends its setting: ' + JSON.stringify(sent));
    power.click();
    expect(page.querySelectorAll('.hotkey').length === 1, 'pin: one shortcut recorder');

    // The window that is pinned is named, and can be let go of; with none pinned there is nothing to let go of.
    const named = page.querySelector('.pin-now');
    expect(named.textContent === S.pin.pinned, 'pin: the pinned window is named: ' + named.textContent);
    const release = [...page.querySelectorAll('.btn')].find((b) => b.textContent === t('pin.now.release'));
    expect(release.style.visibility === 'visible', 'pin: a pinned window can be let go of');
    sent.length = 0;
    release.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'pin_release'), 'pin: let go sends its action');
    S.pin.pinned = null;
    go('pin');
    expect(page.querySelector('.pin-now').textContent === t('pin.now.none'), 'pin: nothing pinned says so');
    expect([...page.querySelectorAll('.btn')].find((b) => b.textContent === t('pin.now.release')).style.visibility === 'hidden', 'pin: nothing to let go of');

    // The permission buttons ask the app.
    sent.length = 0;
    [...page.querySelectorAll('.btn')].find((b) => b.textContent === t('pin.screen.allow')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'pin_screen_access'), 'pin: the Screen Recording button sends its action');
    sent.length = 0;
    [...page.querySelectorAll('.btn')].find((b) => b.textContent === t('pin.access.open')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'open_accessibility'), 'pin: the Accessibility button sends its action');
  },
});
