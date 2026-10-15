// Screen capture: region, window and full screen, text from a region.
// The page of this feature, registered the way the plugin API describes (see web/app.js, PLUGINS).
(window.__plugins = window.__plugins || []).push({
  id: 'capture',
  icon: '<path d="M4 8V6a2 2 0 0 1 2-2h2M16 4h2a2 2 0 0 1 2 2v2M20 16v2a2 2 0 0 1-2 2h-2M8 20H6a2 2 0 0 1-2-2v-2"/><circle cx="12" cy="12" r="3"/>',
  en: {
    'nav.capture': 'Screen capture',
    'page.capture': 'Screen capture',
    'page.capture.sub': 'Capture a region, a window or a whole screen from a shortcut, and read the text in a picture.',
    'capture.enable.title': 'Screen capture',
    'capture.enable.desc': 'Turns on the shortcuts below and the capture entries of the menu bar item. Nothing runs until you press one.',
    'capture.unavailable': 'This version of macOS cannot capture the screen. Screen capture needs macOS 14 or later.',
    'sec.capture.shortcuts': 'Shortcuts',
    'capture.mode.region': 'Region', 'capture.mode.region.desc': 'Drag to choose an area. Hold Space to move it, press Return to confirm and Escape to cancel.',
    'capture.mode.window': 'Window', 'capture.mode.window.desc': 'Click a window to capture it on its own.',
    'capture.mode.screen': 'Screen', 'capture.mode.screen.desc': 'The whole display that has the pointer.',
    'capture.mode.repeat': 'Repeat last region', 'capture.mode.repeat.desc': 'Captures the same area again, with no overlay.',
    'capture.mode.text': 'Text from a region', 'capture.mode.text.desc': 'Choose an area and the text in it goes to the clipboard.',
    'capture.clear': 'Remove shortcut',
    'sec.capture.after': 'After a capture',
    'capture.copy.title': 'Copy to the clipboard', 'capture.copy.desc': 'The picture is ready to paste.',
    'capture.save.title': 'Save to a folder', 'capture.save.desc': 'A PNG named after the date and time. A file that already exists is never replaced.',
    'capture.folder.title': 'Folder', 'capture.folder.choose': 'Choose folder', 'capture.folder.reset': 'Use default',
    'capture.edit.title': 'Open the editor', 'capture.edit.desc': 'Annotate the picture as soon as it is taken.',
    'capture.shadow.title': 'Window shadow', 'capture.shadow.desc': 'Keep the shadow around a window that is captured on its own.',
    'sec.capture.permission': 'Permission',
    'capture.perm.title': 'Screen Recording', 'capture.perm.desc': 'macOS asks for this once. Figghost is signed ad hoc, so macOS asks again after each update.',
    'capture.perm.granted': 'Allowed', 'capture.perm.missing': 'Not allowed', 'capture.perm.allow': 'Allow',
    'sec.capture.guide': 'Using the same keys as macOS',
    'capture.guide.title': 'Free the system shortcuts first',
    'capture.guide.1': 'Open System Settings, then Keyboard, then Keyboard Shortcuts.',
    'capture.guide.2': 'Choose Screenshots in the list and turn off the shortcuts you want to reuse.',
    'capture.guide.3': 'Come back here and record the same keys for the mode you want.',
    'capture.guide.note': 'The system uses Command and Shift with 3, 4 or 5. Figghost starts with Control, Option and Command with 3 to 7, so they never clash.',
    'fault.folder_invalid': 'That folder cannot be used. Choose an ordinary folder that you can write to.',
    'target.capture:region': 'the Capture region shortcut',
    'target.capture:window': 'the Capture window shortcut',
    'target.capture:screen': 'the Capture screen shortcut',
    'target.capture:repeat': 'the Repeat last region shortcut',
    'target.capture:text': 'the Capture text shortcut',
  },
  vi: {
    'nav.capture': 'Chụp màn hình',
    'page.capture': 'Chụp màn hình',
    'page.capture.sub': 'Chụp một vùng, một cửa sổ hoặc cả màn hình bằng phím tắt, và đọc chữ trong ảnh.',
    'capture.enable.title': 'Chụp màn hình',
    'capture.enable.desc': 'Bật các phím tắt bên dưới và các mục chụp ở menu trên thanh menu. Không có gì chạy cho đến khi bạn nhấn một phím tắt.',
    'capture.unavailable': 'Phiên bản macOS này không chụp được màn hình. Chụp màn hình cần macOS 14 trở lên.',
    'sec.capture.shortcuts': 'Phím tắt',
    'capture.mode.region': 'Vùng chọn', 'capture.mode.region.desc': 'Kéo để chọn một vùng. Giữ Space để di chuyển vùng, nhấn Return để xác nhận và Escape để hủy.',
    'capture.mode.window': 'Cửa sổ', 'capture.mode.window.desc': 'Bấm vào một cửa sổ để chụp riêng cửa sổ đó.',
    'capture.mode.screen': 'Toàn màn hình', 'capture.mode.screen.desc': 'Cả màn hình đang có con trỏ chuột.',
    'capture.mode.repeat': 'Chụp lại vùng trước', 'capture.mode.repeat.desc': 'Chụp lại đúng vùng vừa chọn, không hiện lớp phủ.',
    'capture.mode.text': 'Văn bản từ một vùng', 'capture.mode.text.desc': 'Chọn một vùng, chữ trong vùng đó sẽ được chép vào clipboard.',
    'capture.clear': 'Xóa phím tắt',
    'sec.capture.after': 'Sau khi chụp',
    'capture.copy.title': 'Sao chép vào clipboard', 'capture.copy.desc': 'Ảnh sẵn sàng để dán.',
    'capture.save.title': 'Lưu vào thư mục', 'capture.save.desc': 'Tệp PNG đặt tên theo ngày và giờ. Tệp đã có sẽ không bao giờ bị ghi đè.',
    'capture.folder.title': 'Thư mục', 'capture.folder.choose': 'Chọn thư mục', 'capture.folder.reset': 'Dùng mặc định',
    'capture.edit.title': 'Mở trình chỉnh sửa', 'capture.edit.desc': 'Chú thích ảnh ngay sau khi chụp.',
    'capture.shadow.title': 'Bóng cửa sổ', 'capture.shadow.desc': 'Giữ bóng đổ quanh cửa sổ khi chụp riêng cửa sổ.',
    'sec.capture.permission': 'Quyền truy cập',
    'capture.perm.title': 'Ghi màn hình', 'capture.perm.desc': 'macOS chỉ hỏi một lần. Figghost được ký tạm thời nên macOS sẽ hỏi lại sau mỗi lần cập nhật.',
    'capture.perm.granted': 'Đã cho phép', 'capture.perm.missing': 'Chưa cho phép', 'capture.perm.allow': 'Cho phép',
    'sec.capture.guide': 'Dùng cùng phím với macOS',
    'capture.guide.title': 'Hãy giải phóng phím tắt của hệ thống trước',
    'capture.guide.1': 'Mở Cài đặt hệ thống, chọn Bàn phím, rồi Phím tắt bàn phím.',
    'capture.guide.2': 'Chọn Ảnh chụp màn hình trong danh sách và tắt các phím tắt bạn muốn dùng lại.',
    'capture.guide.3': 'Quay lại đây và ghi đúng những phím đó cho chế độ bạn muốn.',
    'capture.guide.note': 'Hệ thống dùng Command và Shift với 3, 4 hoặc 5. Figghost mặc định dùng Control, Option và Command với 3 đến 7 nên hai bên không bao giờ trùng nhau.',
    'fault.folder_invalid': 'Không dùng được thư mục này. Hãy chọn một thư mục thông thường mà bạn có quyền ghi.',
    'target.capture:region': 'phím tắt Chụp vùng chọn',
    'target.capture:window': 'phím tắt Chụp cửa sổ',
    'target.capture:screen': 'phím tắt Chụp toàn màn hình',
    'target.capture:repeat': 'phím tắt Chụp lại vùng trước',
    'target.capture:text': 'phím tắt Chụp văn bản',
  },
  build() {
    const shortcutRow = (key) => {
      const target = 'capture:' + key;
      const title = t('capture.mode.' + key);
      const clear = h('button', { class: 'btn icon', 'aria-label': t('capture.clear') + ': ' + title, title: t('capture.clear'), onclick: () => send({ t: 'hotkey', target, value: '' }) }, icon('close'));
      reg(() => { clear.style.visibility = shortcutOf(target) ? 'visible' : 'hidden'; });
      return row(title, t('capture.mode.' + key + '.desc'), [hotkeyButton(target, { allowClear: true, label: title }), clear], { when: () => S.capture.enabled });
    };

    const unavailable = h('div', { class: 'banner' }, h('div', { class: 'grow', text: t('capture.unavailable') }));
    reg(() => { unavailable.style.display = S.capture.available ? 'none' : ''; });

    const reset = button(t('capture.folder.reset'), () => send({ t: 'set', path: 'capture.folder', value: '' }), { small: true });
    const folder = row(t('capture.folder.title'), ' ', [
      reset, button(t('capture.folder.choose'), () => send({ t: 'action', name: 'capture_choose_folder' }), { icon: 'folder', small: true }),
    ], { when: () => S.capture.enabled && S.capture.save });
    const path = $('.row-desc', folder);
    path.classList.add('capture-path');
    reg(() => { path.textContent = S.capture.folder; reset.style.display = S.capture.custom_folder ? '' : 'none'; });

    const allow = button(t('capture.perm.allow'), () => send({ t: 'action', name: 'capture_permission' }), { small: true });
    reg(() => { allow.style.display = S.capture.permission ? 'none' : ''; });
    const permission = row(t('capture.perm.title'), t('capture.perm.desc'), [
      pill(() => (S.capture.permission ? t('capture.perm.granted') : t('capture.perm.missing')), () => S.capture.permission),
      allow,
    ]);

    const steps = h('ol', { class: 'capture-steps' }, [1, 2, 3].map((n) => h('li', { text: t('capture.guide.' + n) })));
    const guide = row(t('capture.guide.title'), t('capture.guide.note'), null, { block: true });
    $('.row-text', guide).append(steps);

    return [
      card(toggleRow('capture.enabled', t('capture.enable.title'), t('capture.enable.desc'))),
      unavailable,
      sectionTitle(t('sec.capture.shortcuts')),
      card(...['region', 'window', 'screen', 'repeat', 'text'].map(shortcutRow)),
      sectionTitle(t('sec.capture.after')),
      card(
        toggleRow('capture.copy', t('capture.copy.title'), t('capture.copy.desc'), { when: () => S.capture.enabled }),
        toggleRow('capture.save', t('capture.save.title'), t('capture.save.desc'), { when: () => S.capture.enabled }),
        folder,
        toggleRow('capture.edit', t('capture.edit.title'), t('capture.edit.desc'), { when: () => S.capture.enabled }),
        toggleRow('capture.window_shadow', t('capture.shadow.title'), t('capture.shadow.desc'), { when: () => S.capture.enabled })),
      sectionTitle(t('sec.capture.permission')),
      card(permission),
      sectionTitle(t('sec.capture.guide')),
      card(guide),
    ];
  },
  selftest({ expect, go, sent, S, $ }) {
    go('capture');
    const root = $('#scroller');
    const switches = () => [...root.querySelectorAll('.switch')];

    // The enable switch writes its setting.
    sent.length = 0;
    const was = S.capture.enabled;
    switches()[0].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'capture.enabled' && m.value === !was), 'the capture switch sends its setting: ' + JSON.stringify(sent));
    switches()[0].click();

    // One recorder per mode, each aimed at its own target, each removable.
    const recorders = [...root.querySelectorAll('.hotkey')];
    expect(recorders.length === 5, 'five shortcut recorders, found ' + recorders.length);
    sent.length = 0;
    recorders[0].click();
    expect(sent.some((m) => m.t === 'recording' && m.on === true), 'a recorder starts recording');
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'Digit8', metaKey: true, altKey: true, ctrlKey: true, bubbles: true, cancelable: true }));
    expect(sent.some((m) => m.t === 'hotkey' && m.target === 'capture:region' && m.value === 'ctrl+opt+cmd+8'), 'the region recorder sends its target: ' + JSON.stringify(sent));
    sent.length = 0;
    const clears = [...root.querySelectorAll('.btn.icon')];
    expect(clears.length === 5, 'a clear button per recorder');
    clears[2].click();
    expect(sent.some((m) => m.t === 'hotkey' && m.target === 'capture:screen' && m.value === ''), 'clearing sends an empty shortcut: ' + JSON.stringify(sent));

    // Switches under "After a capture" write their own settings.
    for (const [index, path] of [[1, 'capture.copy'], [2, 'capture.save'], [3, 'capture.edit'], [4, 'capture.window_shadow']]) {
      sent.length = 0;
      switches()[index].click();
      expect(sent.some((m) => m.t === 'set' && m.path === path), `the ${path} switch sends its setting: ` + JSON.stringify(sent));
      switches()[index].click();
    }

    // The folder and the permission buttons are actions of this feature.
    sent.length = 0;
    const named = (text) => [...root.querySelectorAll('button')].find((b) => b.textContent.trim() === text);
    named(t('capture.folder.choose')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'capture_choose_folder'), 'the folder button asks the app to choose');
    S.capture.permission = false;
    go('capture');
    sent.length = 0;
    named(t('capture.perm.allow')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'capture_permission'), 'the permission button asks the app');
    S.capture.permission = true;
    go('capture');
    expect(named(t('capture.perm.allow')).style.display === 'none', 'the permission button is hidden once it is granted');
    expect(root.textContent.includes(t('capture.perm.granted')), 'the permission state is shown');
    S.capture.permission = false;
    S.capture.available = false;
    go('capture');
    expect([...root.querySelectorAll('.banner')].some((b) => b.style.display !== 'none' && b.textContent.includes('macOS 14')), 'a system that cannot capture says so');
    S.capture.available = true;
    go('capture');
  },
});
