// Notifications: the notifications of other apps on the notch, read from the store of Notification Center. They arrive
// through the system, as macOS's banners with the tab keeping the list, or through the notch. The reading runs in the
// app (features/notifications in Rust).
(function () {
  const on = () => S.notifications.enabled;
  const onNotch = () => on() && S.notifications.mode === 'notch';

  function accessRow() {
    const status = h('span', { class: 'row-desc' });
    const open = button(t('notifications.access.open'), () => send({ t: 'action', name: 'notifications_access' }), { small: true });
    reg(() => {
      // Null until the feature is on, as the store is not touched before.
      const access = S.notifications.access;
      status.textContent = access === true ? t('notifications.access.ok') : access === false ? t('notifications.access.no') : t('notifications.access.unknown');
      // The button's own style decides its display, so `hidden` alone would not hide it.
      open.style.display = access === true ? 'none' : '';
    });
    return h('div', { class: 'row' },
      h('div', { class: 'row-text' }, h('div', { class: 'row-title' }, t('notifications.access.title')), status),
      h('div', { class: 'row-control' }, open));
  }

  (window.__plugins = window.__plugins || []).push({
    id: 'notifications',
    icon: 'bell.badge.fill',
    en: {
      'nav.notifications': 'Notifications', 'page.notifications': 'Notifications',
      'page.notifications.sub': 'Receive the notifications of your apps on the notch, or keep them in the system and find them on a tab of the notch.',
      'notifications.enable.title': 'Show notifications on the notch', 'notifications.enable.desc': 'The notifications your apps send are read from Notification Center as they come and listed on a Notifications tab, newest first; a click on one opens its app.',
      'notifications.access.title': 'Full Disk Access', 'notifications.access.ok': 'Allowed: notifications can be read.', 'notifications.access.no': 'Not allowed yet. macOS keeps notifications where only an app with Full Disk Access may read them: turn Figghost on in the list, then quit and open it again.', 'notifications.access.open': 'Open the list', 'notifications.access.unknown': 'Checked once the feature is on: macOS keeps notifications where only an app with Full Disk Access may read them, and the button opens that list.',
      'sec.notifications.how': 'How they arrive',
      'notifications.mode.title': 'Receive notifications', 'notifications.mode.desc': 'Through the system: macOS shows its banners as before, and the tab keeps the list. Through the notch: each one comes out of the closed notch at once, ahead of the music and the date and time, with the app\'s icon, its title and its words.',
      'notifications.mode.system': 'Through the system', 'notifications.mode.notch': 'Through the notch',
      'notifications.banners.title': 'Turn off macOS\'s banners', 'notifications.banners.desc': 'So that a notification does not show twice: in System Settings, Notifications, set the alert style of each app whose notifications should come through the notch to None. They still reach Notification Center, and so the notch. Do Not Disturb does the same for every app at once.', 'notifications.banners.open': 'Open Notifications',
      'notifications.seconds.title': 'Time on the notch', 'notifications.seconds.desc': 'How long a notification stays on the closed notch; a newer one takes its place.', 'notifications.seconds.n': '{n} seconds',
      'notifications.preview.title': 'Show what they say', 'notifications.preview.desc': 'Off shows only the app and New notification, for a notch that others can see.',
      'sec.notifications.privacy': 'Privacy',
      'notifications.privacy.title': 'Read, never written or sent', 'notifications.privacy.desc': 'Figghost only reads the store macOS keeps, and only when it changes. Nothing is written to it, nothing is kept on disk and nothing leaves this Mac; the list goes when Figghost quits.',
    },
    vi: {
      'nav.notifications': 'Thông báo', 'page.notifications': 'Thông báo',
      'page.notifications.sub': 'Nhận thông báo của các ứng dụng ngay trên notch, hoặc để hệ thống hiện như cũ và xem lại trong một tab của notch.',
      'notifications.enable.title': 'Hiện thông báo trên notch', 'notifications.enable.desc': 'Thông báo các ứng dụng gửi tới được đọc từ Trung tâm thông báo ngay khi tới và liệt kê trong tab Thông báo, mới nhất ở trên; bấm vào một thông báo để mở ứng dụng đã gửi nó.',
      'notifications.access.title': 'Truy cập toàn bộ ổ đĩa', 'notifications.access.ok': 'Đã cho phép: đọc được thông báo.', 'notifications.access.no': 'Chưa cho phép. macOS cất thông báo ở chỗ chỉ ứng dụng có quyền Truy cập toàn bộ ổ đĩa mới đọc được: hãy bật Figghost trong danh sách, rồi thoát và mở lại app.', 'notifications.access.open': 'Mở danh sách', 'notifications.access.unknown': 'Sẽ kiểm tra khi bật tính năng: macOS cất thông báo ở chỗ chỉ ứng dụng có quyền Truy cập toàn bộ ổ đĩa mới đọc được, và nút này mở đúng danh sách đó.',
      'sec.notifications.how': 'Cách nhận thông báo',
      'notifications.mode.title': 'Nhận thông báo', 'notifications.mode.desc': 'Qua hệ thống: macOS vẫn hiện banner như cũ, tab giữ danh sách. Qua notch: mỗi thông báo hiện ngay trên notch đóng, lấn cả nhạc lẫn ngày giờ, kèm icon ứng dụng, tiêu đề và nội dung.',
      'notifications.mode.system': 'Qua hệ thống', 'notifications.mode.notch': 'Qua notch',
      'notifications.banners.title': 'Tắt banner của macOS', 'notifications.banners.desc': 'Để một thông báo không hiện hai lần: trong Cài đặt hệ thống, mục Thông báo, đặt kiểu thông báo của từng ứng dụng muốn nhận qua notch là Không có. Thông báo vẫn vào Trung tâm thông báo, nên vẫn lên notch. Bật Không làm phiền thì có tác dụng tương tự cho mọi ứng dụng cùng lúc.', 'notifications.banners.open': 'Mở mục Thông báo',
      'notifications.seconds.title': 'Thời gian hiện trên notch', 'notifications.seconds.desc': 'Một thông báo nằm trên notch đóng bao lâu; thông báo mới hơn sẽ thế chỗ.', 'notifications.seconds.n': '{n} giây',
      'notifications.preview.title': 'Hiện nội dung', 'notifications.preview.desc': 'Tắt thì notch chỉ hiện tên ứng dụng và chữ Thông báo mới, phù hợp khi người khác nhìn thấy màn hình.',
      'sec.notifications.privacy': 'Riêng tư',
      'notifications.privacy.title': 'Chỉ đọc, không ghi, không gửi đi', 'notifications.privacy.desc': 'Figghost chỉ đọc kho thông báo của macOS, và chỉ khi kho thay đổi. Không ghi gì vào đó, không lưu gì xuống ổ đĩa và không gửi gì ra khỏi máy; danh sách mất khi Figghost thoát.',
    },
    build() {
      const seconds = () => [3, 5, 8, 12].map((n) => ({ value: n, label: t('notifications.seconds.n', { n }) }));
      const banners = button(t('notifications.banners.open'), () => send({ t: 'action', name: 'notifications_banners' }), { small: true });
      return [
        card(toggleRow('notifications.enabled', t('notifications.enable.title'), t('notifications.enable.desc'))),
        card(accessRow()),
        sectionTitle(t('sec.notifications.how')),
        card(
          segRow('notifications.mode', t('notifications.mode.title'), t('notifications.mode.desc'), ['system', 'notch'], { when: on }),
          row(t('notifications.banners.title'), t('notifications.banners.desc'), banners, { when: onNotch }),
          selectRow({ path: 'notifications.seconds', title: t('notifications.seconds.title'), desc: t('notifications.seconds.desc'), options: seconds, when: onNotch }),
          toggleRow('notifications.preview', t('notifications.preview.title'), t('notifications.preview.desc'), { when: onNotch })),
        sectionTitle(t('sec.notifications.privacy')),
        card(infoRow(t('notifications.privacy.title'), t('notifications.privacy.desc'))),
      ];
    },
    selftest({ expect, go, sent, S }) {
      go('notifications');
      const page = document.querySelector('#scroller');
      const switches = [...page.querySelectorAll('.switch')];
      expect(switches.length === 2, 'notifications: a switch for the feature and one for what they say');
      sent.length = 0;
      const was = S.notifications.enabled;
      switches[0].click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'notifications.enabled' && m.value === !was), 'notifications: the switch sends its setting: ' + JSON.stringify(sent));
      switches[0].click();
      sent.length = 0;
      const system = [...page.querySelectorAll('.seg button')].find((b) => b.textContent === t('notifications.mode.system'));
      system.click();
      expect(sent.some((m) => m.path === 'notifications.mode' && m.value === 'system'), 'notifications: the way they arrive is a word: ' + JSON.stringify(sent));
      [...page.querySelectorAll('.seg button')].find((b) => b.textContent === t('notifications.mode.notch')).click();
      sent.length = 0;
      [...page.querySelectorAll('button')].find((b) => b.textContent === t('notifications.access.open')).click();
      [...page.querySelectorAll('button')].find((b) => b.textContent === t('notifications.banners.open')).click();
      expect(sent.some((m) => m.t === 'action' && m.name === 'notifications_access') && sent.some((m) => m.t === 'action' && m.name === 'notifications_banners'), 'notifications: the buttons open System Settings: ' + JSON.stringify(sent));
      expect(page.querySelectorAll('select.pick').length === 1, 'notifications: a choice of the seconds on the notch');
    },
  });
})();
