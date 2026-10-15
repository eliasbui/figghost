// Screenshots and recordings: what happens after a capture, the shortcuts of each kind, and what a
// recording includes. The capture itself runs in the app (features/capture in Rust).
(function () {
  const KINDS = ['area', 'window', 'screen', 'timer', 'text', 'scroll', 'record', 'gif'];
  const RECORD = ['system_audio', 'microphone', 'camera', 'clicks', 'keys'];
  const on = () => S.capture.enabled;

  function accessRow() {
    const status = h('span', { class: 'row-desc' });
    const ask = button(t('capture.access.ask'), () => send({ t: 'action', name: 'capture_screen_access' }), { small: true });
    reg(() => {
      status.textContent = S.capture.screen_access ? t('capture.access.ok') : t('capture.access.no');
      // The button's own style decides its display, so `hidden` alone would not hide it.
      ask.style.display = S.capture.screen_access ? 'none' : '';
    });
    return h('div', { class: 'row' },
      h('div', { class: 'row-text' }, h('div', { class: 'row-title' }, t('capture.access.title')), status),
      h('div', { class: 'row-control' }, ask));
  }

  function folderRow() {
    const path = h('span', { class: 'row-desc' });
    reg(() => { path.textContent = S.capture.folder_path; });
    const open = button(t('capture.folder.open'), () => send({ t: 'action', name: 'capture_open_folder' }), { small: true });
    return h('div', { class: 'row' },
      h('div', { class: 'row-text' }, h('div', { class: 'row-title' }, t('capture.folder.title')), path),
      h('div', { class: 'row-control' }, open));
  }

  (window.__plugins = window.__plugins || []).push({
    id: 'capture',
    icon: 'camera.viewfinder',
    en: {
      'nav.capture': 'Capture',
      'page.capture': 'Screenshots and recordings',
      'page.capture.sub': 'Areas, windows and displays, text, long pages, and videos or GIFs of the screen.',
      'capture.enable.title': 'Screenshots and recordings',
      'capture.enable.desc': 'The shortcuts below and the Capture menu of the menu bar item.',
      'capture.access.title': 'Screen Recording access',
      'capture.access.ok': 'Allowed.',
      'capture.access.no': 'Needed for every capture. Allow Figghost in System Settings, then capture again.',
      'capture.access.ask': 'Allow...',
      'sec.capture.after': 'After a capture',
      'capture.save.title': 'Save to the folder',
      'capture.save.desc': 'Off keeps captures in a temporary folder, for the quick panel only.',
      'capture.folder.title': 'Folder',
      'capture.folder.open': 'Open',
      'capture.copy.title': 'Copy',
      'capture.copy.desc': 'Pictures go to the clipboard as images, recordings as files.',
      'capture.quick.title': 'Quick panel',
      'capture.quick.desc': 'A thumbnail in the corner with Copy, Save, Edit and Pin. Drag it into another app to drop the file there.',
      'capture.quick_seconds.title': 'The quick panel stays',
      'capture.quick_seconds.forever': 'until closed',
      'capture.timer.title': 'Countdown of a timed capture',
      'capture.seconds': '{n} s',
      'sec.capture.shortcuts': 'Shortcuts',
      'capture.kind.area': 'Area or window',
      'capture.kind.window': 'Window',
      'capture.kind.screen': 'Whole screen',
      'capture.kind.timer': 'Screen after the countdown',
      'capture.kind.text': 'Copy the text of an area',
      'capture.kind.scroll': 'Scrolling capture',
      'capture.kind.record': 'Record the screen, again to stop',
      'capture.kind.gif': 'Record a GIF, again to stop',
      'sec.capture.record': 'Recordings',
      'capture.record.unsupported': 'Recording needs macOS 15 or later.',
      'capture.record.system_audio.title': 'Computer audio',
      'capture.record.system_audio.desc': 'What the apps play, as the Sound tab sets it.',
      'capture.record.microphone.title': 'Microphone',
      'capture.record.microphone.desc': 'macOS asks for access the first time.',
      'capture.record.camera.title': 'Camera',
      'capture.record.camera.desc': 'A round bubble in a corner of the recording; drag it to move it.',
      'capture.record.clicks.title': 'Show clicks',
      'capture.record.clicks.desc': 'A ring where the mouse is clicked.',
      'capture.record.keys.title': 'Show shortcuts pressed',
      'capture.record.keys.desc': 'Shortcuts and keys such as Return appear at the bottom. Plain typing never does, so passwords stay out.',
    },
    vi: {
      'nav.capture': 'Chụp màn hình',
      'page.capture': 'Chụp và quay màn hình',
      'page.capture.sub': 'Vùng, cửa sổ và màn hình, chữ, trang dài, và video hoặc GIF của màn hình.',
      'capture.enable.title': 'Chụp và quay màn hình',
      'capture.enable.desc': 'Các phím tắt bên dưới và menu Chụp màn hình của biểu tượng trên thanh menu.',
      'capture.access.title': 'Quyền Ghi màn hình',
      'capture.access.ok': 'Đã cho phép.',
      'capture.access.no': 'Cần cho mọi lần chụp. Cho phép Figghost trong Cài đặt hệ thống rồi chụp lại.',
      'capture.access.ask': 'Cho phép...',
      'sec.capture.after': 'Sau khi chụp',
      'capture.save.title': 'Lưu vào thư mục',
      'capture.save.desc': 'Tắt thì ảnh chụp nằm trong thư mục tạm, chỉ dùng cho bảng xem nhanh.',
      'capture.folder.title': 'Thư mục',
      'capture.folder.open': 'Mở',
      'capture.copy.title': 'Sao chép',
      'capture.copy.desc': 'Ảnh vào bộ nhớ tạm dạng ảnh, bản quay dạng tệp.',
      'capture.quick.title': 'Bảng xem nhanh',
      'capture.quick.desc': 'Ảnh nhỏ ở góc màn hình với Sao chép, Lưu, Sửa và Ghim. Kéo ảnh vào ứng dụng khác để thả tệp vào đó.',
      'capture.quick_seconds.title': 'Bảng xem nhanh ở lại',
      'capture.quick_seconds.forever': 'đến khi đóng',
      'capture.timer.title': 'Đếm ngược khi chụp hẹn giờ',
      'capture.seconds': '{n} giây',
      'sec.capture.shortcuts': 'Phím tắt',
      'capture.kind.area': 'Vùng hoặc cửa sổ',
      'capture.kind.window': 'Cửa sổ',
      'capture.kind.screen': 'Cả màn hình',
      'capture.kind.timer': 'Cả màn hình sau khi đếm ngược',
      'capture.kind.text': 'Sao chép chữ trong một vùng',
      'capture.kind.scroll': 'Chụp cuộn',
      'capture.kind.record': 'Quay màn hình, bấm lại để dừng',
      'capture.kind.gif': 'Quay GIF, bấm lại để dừng',
      'sec.capture.record': 'Quay màn hình',
      'capture.record.unsupported': 'Quay màn hình cần macOS 15 trở lên.',
      'capture.record.system_audio.title': 'Âm thanh máy tính',
      'capture.record.system_audio.desc': 'Âm thanh các ứng dụng phát ra, theo cài đặt của tab Âm thanh.',
      'capture.record.microphone.title': 'Micrô',
      'capture.record.microphone.desc': 'macOS hỏi quyền ở lần đầu.',
      'capture.record.camera.title': 'Camera',
      'capture.record.camera.desc': 'Một bong bóng tròn ở góc bản quay; kéo để di chuyển.',
      'capture.record.clicks.title': 'Hiện cú nhấp chuột',
      'capture.record.clicks.desc': 'Một vòng tròn ở chỗ nhấp chuột.',
      'capture.record.keys.title': 'Hiện phím tắt đã bấm',
      'capture.record.keys.desc': 'Phím tắt và các phím như Return hiện ở phía dưới. Chữ gõ thường không bao giờ hiện, nên mật khẩu không lọt vào bản quay.',
    },
    build() {
      return [
        card(toggleRow('capture.enabled', t('capture.enable.title'), t('capture.enable.desc')), accessRow()),
        sectionTitle(t('sec.capture.after')),
        card(
          toggleRow('capture.save', t('capture.save.title'), t('capture.save.desc'), { when: on }),
          folderRow(),
          toggleRow('capture.copy', t('capture.copy.title'), t('capture.copy.desc'), { when: on }),
          toggleRow('capture.quick_panel', t('capture.quick.title'), t('capture.quick.desc'), { when: on }),
          sliderRow({
            path: 'capture.quick_seconds', title: t('capture.quick_seconds.title'), min: 0, max: 30, step: 1,
            format: (v) => (v === 0 ? t('capture.quick_seconds.forever') : t('capture.seconds', { n: v })),
          }),
          sliderRow({
            path: 'capture.timer_seconds', title: t('capture.timer.title'), min: 1, max: 30, step: 1,
            format: (v) => t('capture.seconds', { n: v }),
          })),
        sectionTitle(t('sec.capture.shortcuts')),
        card(...KINDS.map((kind) => row(t('capture.kind.' + kind), null,
          hotkeyButton('capture:' + kind, { allowClear: true, label: t('capture.kind.' + kind) }), { when: on }))),
        sectionTitle(t('sec.capture.record')),
        card(
          ...(S.capture.recording_supported ? [] : [infoRow(t('capture.record.unsupported'), null)]),
          ...RECORD.map((key) => toggleRow('capture.record.' + key, t(`capture.record.${key}.title`),
            t(`capture.record.${key}.desc`), { when: () => on() && S.capture.recording_supported }))),
      ];
    },
    selftest({ expect, go, sent, S }) {
      go('capture');
      const page = document.querySelector('#scroller');
      expect(page.querySelectorAll('.hotkey').length === KINDS.length, 'capture: a shortcut button per kind');
      sent.length = 0;
      const keys = [...page.querySelectorAll('.switch')].find((s) => s.getAttribute('aria-label') === t('capture.record.keys.title'));
      const was = S.capture.record.keys;
      keys.click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'capture.record.keys' && m.value === !was), 'capture: a recording switch sends its setting: ' + JSON.stringify(sent));
      keys.click();
      sent.length = 0;
      [...page.querySelectorAll('button')].find((b) => b.textContent === t('capture.folder.open')).click();
      expect(sent.some((m) => m.t === 'action' && m.name === 'capture_open_folder'), 'capture: the folder button asks the app to open it');
      expect(page.textContent.includes(S.capture.folder_path), 'capture: the folder is shown');
    },
  });
})();
