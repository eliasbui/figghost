// Vietnamese typing: Telex or VNI in every app, with the engine of Gõ Nhanh.
(window.__plugins = window.__plugins || []).push({
  id: 'typing',
  icon: 'keyboard',
  en: {
    'nav.typing': 'Vietnamese typing', 'page.typing': 'Vietnamese typing',
    'page.typing.sub': 'Type Vietnamese in every app with Telex or VNI, with no other input method. English words you type come back by themselves.',
    'typing.enable.title': 'Type Vietnamese', 'typing.enable.desc': 'The marks go where they belong as you type. Needs Accessibility, like scrolling and window snapping.',
    'typing.now.title': 'Typing in', 'typing.now.desc': 'The shortcut, the menu bar panel and this button switch between Vietnamese and English.',
    'typing.now.vi': 'Vietnamese', 'typing.now.en': 'English', 'typing.now.switch': 'Switch',
    'typing.other.title': '{name} is running',
    'typing.other.desc': 'Two input methods would put every mark twice, so Figghost leaves the keys alone while {name} runs. Quit one of the two.',
    'sec.typing.how': 'How you type',
    'typing.method': 'Typing method', 'typing.method.desc': 'Telex types the marks with letters (aa is â, s the acute accent); VNI with digits (a6 is â, 1 the acute accent).',
    'typing.method.telex': 'Telex', 'typing.method.vni': 'VNI',
    'typing.modern.title': 'Marks where the new rules put them', 'typing.modern.desc': 'hoà, khoẻ, thuỷ. Off writes hòa, khỏe, thủy.',
    'typing.english.title': 'Give English words back', 'typing.english.desc': 'text, with or push typed with Telex come back as they were when the word ends, instead of tẽt, ưith or púh.',
    'typing.shortcut': 'Shortcut', 'typing.shortcut.desc': 'Switches between Vietnamese and English in any app. Press Delete while recording to remove it.',
    'target.typing': 'the Vietnamese typing shortcut',
    'sec.typing.apps': 'Apps left alone',
    'typing.apps.title': 'Apps where every key is left alone', 'typing.apps.desc': 'Typing in these apps stays as it is. iPhone Mirroring and remote desktops are always left alone, since their keys go to another device.',
    'typing.apps.add': 'Add app', 'typing.apps.empty': 'No app is left alone.',
    'typing.access.title': 'Accessibility', 'typing.access.desc': 'Needed to read the keys and type the marks.', 'typing.access.open': 'Open settings',
    'typing.engine.title': 'The engine', 'typing.engine.desc': 'The marks are put by the engine of Gõ Nhanh (copyright 2025 Gõ Nhanh Contributors, BSD 3-Clause License), which Figghost uses unchanged. Its licence is on the About page.',
  },
  vi: {
    'nav.typing': 'Gõ tiếng Việt', 'page.typing': 'Gõ tiếng Việt',
    'page.typing.sub': 'Gõ tiếng Việt trong mọi ứng dụng bằng Telex hoặc VNI, không cần bộ gõ nào khác. Từ tiếng Anh bạn gõ sẽ tự trở lại như cũ.',
    'typing.enable.title': 'Gõ tiếng Việt', 'typing.enable.desc': 'Dấu được đặt đúng chỗ ngay khi bạn gõ. Cần quyền Trợ năng, như cuộn chuột và xếp cửa sổ.',
    'typing.now.title': 'Đang gõ', 'typing.now.desc': 'Phím tắt, bảng ở thanh menu và nút này chuyển qua lại giữa tiếng Việt và tiếng Anh.',
    'typing.now.vi': 'Tiếng Việt', 'typing.now.en': 'Tiếng Anh', 'typing.now.switch': 'Chuyển',
    'typing.other.title': '{name} đang chạy',
    'typing.other.desc': 'Hai bộ gõ cùng chạy sẽ bỏ dấu hai lần, nên Figghost để nguyên các phím khi {name} đang chạy. Hãy thoát một trong hai.',
    'sec.typing.how': 'Cách gõ',
    'typing.method': 'Kiểu gõ', 'typing.method.desc': 'Telex bỏ dấu bằng chữ (aa là â, s là dấu sắc); VNI bằng số (a6 là â, 1 là dấu sắc).',
    'typing.method.telex': 'Telex', 'typing.method.vni': 'VNI',
    'typing.modern.title': 'Đặt dấu theo quy tắc mới', 'typing.modern.desc': 'hoà, khoẻ, thuỷ. Tắt thì viết hòa, khỏe, thủy.',
    'typing.english.title': 'Trả lại từ tiếng Anh', 'typing.english.desc': 'text, with hay push gõ bằng Telex sẽ trở lại như cũ khi hết từ, thay vì tẽt, ưith hay púh.',
    'typing.shortcut': 'Phím tắt', 'typing.shortcut.desc': 'Chuyển qua lại giữa tiếng Việt và tiếng Anh ở mọi ứng dụng. Nhấn Delete khi đang ghi phím để xóa.',
    'target.typing': 'phím tắt Gõ tiếng Việt',
    'sec.typing.apps': 'Ứng dụng giữ nguyên phím',
    'typing.apps.title': 'Ứng dụng giữ nguyên mọi phím', 'typing.apps.desc': 'Gõ trong các ứng dụng này luôn giữ nguyên. iPhone Mirroring và điều khiển máy từ xa luôn được giữ nguyên, vì phím của chúng đi tới thiết bị khác.',
    'typing.apps.add': 'Thêm ứng dụng', 'typing.apps.empty': 'Chưa có ứng dụng nào.',
    'typing.access.title': 'Trợ năng', 'typing.access.desc': 'Cần để đọc phím và gõ dấu.', 'typing.access.open': 'Mở cài đặt',
    'typing.engine.title': 'Bộ máy gõ', 'typing.engine.desc': 'Dấu được đặt bằng bộ máy của Gõ Nhanh (bản quyền 2025 Gõ Nhanh Contributors, giấy phép BSD 3-Clause), Figghost dùng nguyên bản. Giấy phép của nó ở trang Giới thiệu.',
  },
  build() {
    const on = () => S.typing.enabled;
    const when = { when: on };

    const otherTitle = h('span');
    const otherDesc = h('div', { class: 'row-desc' });
    const otherCard = card(h('div', { class: 'row' }, h('div', { class: 'row-text' }, h('div', { class: 'row-title' }, otherTitle), otherDesc)));
    reg(() => {
      const name = S.typing.other;
      otherCard.hidden = !(on() && name);
      if (name) {
        otherTitle.textContent = t('typing.other.title', { name });
        otherDesc.textContent = t('typing.other.desc', { name });
      }
    });

    const now = pill(() => (S.typing.vietnamese ? t('typing.now.vi') : t('typing.now.en')), () => S.typing.vietnamese);
    const switchBtn = button(t('typing.now.switch'), () => send({ t: 'action', name: 'typing_toggle' }), { small: true });
    const running = { when: () => on() && S.typing.running };

    const appRowOf = (app) => appRow(app, h('button', {
      class: 'btn icon', 'aria-label': t('apps.remove') + ': ' + app.name, title: t('apps.remove'),
      onclick: () => send({ t: 'action', name: 'typing_app_remove:' + app.id }),
    }, icon('close')));

    return [
      card(toggleRow('typing.enabled', t('typing.enable.title'), t('typing.enable.desc'))),
      otherCard,
      card(
        row(t('typing.now.title'), t('typing.now.desc'), [now, switchBtn], running),
        row(t('typing.shortcut'), t('typing.shortcut.desc'), hotkeyButton('typing', { allowClear: true, label: t('typing.shortcut') }), when),
        row(t('typing.access.title'), t('typing.access.desc'), [
          pill(() => (S.typing.accessibility ? t('access.granted') : t('access.missing')), () => S.typing.accessibility),
          button(t('typing.access.open'), () => send({ t: 'action', name: 'open_accessibility' }), { small: true }),
        ], when)),
      sectionTitle(t('sec.typing.how')),
      card(
        segRow('typing.method', t('typing.method'), t('typing.method.desc'), ['telex', 'vni'], when),
        toggleRow('typing.modern_tone', t('typing.modern.title'), t('typing.modern.desc'), when),
        toggleRow('typing.english_restore', t('typing.english.title'), t('typing.english.desc'), when)),
      sectionTitle(t('sec.typing.apps')),
      card(
        row(t('typing.apps.title'), t('typing.apps.desc'), addAppButton('typing', t('typing.apps.add')), when),
        dynamicList(() => S.typing.apps, appRowOf, t('typing.apps.empty'))),
      card(infoRow(t('typing.engine.title'), t('typing.engine.desc'))),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('typing');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const master = page.querySelector('.switch');
    const was = S.typing.enabled;
    master.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'typing.enabled' && m.value === !was), 'typing: the switch sends its setting: ' + JSON.stringify(sent));
    master.click();

    expect(page.querySelectorAll('.hotkey').length === 1, 'typing: one shortcut recorder, for the switch');
    sent.length = 0;
    const vni = [...page.querySelectorAll('.seg button')].find((b) => b.textContent === 'VNI');
    vni.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'typing.method' && m.value === 'vni'), 'typing: the method sends its setting: ' + JSON.stringify(sent));

    const button = (label) => [...page.querySelectorAll('.btn')].find((b) => b.textContent === label);
    sent.length = 0;
    button(t('typing.now.switch')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'typing_toggle'), 'typing: the switch button sends its action');
    sent.length = 0;
    page.querySelector('.app-row .btn.icon').click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'typing_app_remove:com.apple.Terminal'), 'typing: removing an app names it: ' + JSON.stringify(sent));

    // Another input method shows a notice with its name; none shows nothing.
    const notice = () => [...page.querySelectorAll('.card')].find((c) => c.textContent.includes(t('typing.other.title', { name: 'EVKey' })));
    expect(!notice(), 'typing: no notice without another input method');
    S.typing.other = 'EVKey'; runSyncs();
    expect(notice() && !notice().hidden, 'typing: the notice names the other input method');
    S.typing.other = null; runSyncs();
    S.typing.vietnamese = false; runSyncs();
    expect(page.textContent.includes(t('typing.now.en')), 'typing: the language follows what the app says');
    S.typing.vietnamese = true; runSyncs();
  },
});
