// Outlook: the new mail and the calendar reminders of the Outlook app on a tab of the notch, read from the notifications
// it sends, with a card to read one in the middle of the screen. The reading runs in the app (features/outlook in Rust).
(function () {
  const on = () => S.outlook.enabled;

  function accessRow() {
    const status = h('span', { class: 'row-desc' });
    const open = button(t('outlook.access.open'), () => send({ t: 'action', name: 'outlook_access' }), { small: true });
    reg(() => {
      const access = S.outlook.access;
      status.textContent = S.outlook.installed === false ? t('outlook.access.missing')
        : access === true ? t('outlook.access.ok') : access === false ? t('outlook.access.no') : t('outlook.access.unknown');
      open.style.display = access === true ? 'none' : '';
    });
    return h('div', { class: 'row' },
      h('div', { class: 'row-text' }, h('div', { class: 'row-title' }, t('outlook.access.title')), status),
      h('div', { class: 'row-control' }, open));
  }

  (window.__plugins = window.__plugins || []).push({
    id: 'outlook',
    icon: 'envelope.fill',
    en: {
      'nav.outlook': 'Outlook', 'page.outlook': 'Outlook',
      'page.outlook.sub': 'New mail and calendar reminders of Outlook on a tab of the notch, and a card to read one at a glance.',
      'outlook.enable.title': 'Outlook on the notch', 'outlook.enable.desc': 'The notifications the Outlook app sends are listed on an Outlook tab, newest first; a click on one opens it in a card in the middle of the screen. No account is signed in: the sender, the subject and the first lines are what Outlook puts in its notification.',
      'outlook.access.title': 'Full Disk Access', 'outlook.access.ok': 'Allowed: Outlook\'s notifications can be read.', 'outlook.access.no': 'Not allowed yet: turn Figghost on in the list, then quit and open it again.', 'outlook.access.unknown': 'Checked once the feature is on: macOS keeps notifications where only an app with Full Disk Access may read them.', 'outlook.access.missing': 'The Outlook app is not on this Mac.', 'outlook.access.open': 'Open the list',
      'sec.outlook.new': 'When mail comes',
      'outlook.notch.title': 'Show on the notch', 'outlook.notch.desc': 'A new mail comes out of the closed notch for a few seconds, with the sender and the subject.',
      'outlook.seconds.title': 'Time on the notch', 'outlook.seconds.n': '{n} seconds',
      'outlook.preview.title': 'Show what it says', 'outlook.preview.desc': 'Off shows only New mail on the notch, for a notch that others can see.',
      'outlook.drop.title': 'Drop from the notch', 'outlook.drop.desc': 'A new mail falls from the notch as a drop to the middle of the screen and opens into the card to read it. A press anywhere else or Escape puts it away.',
      'outlook.try': 'Try it',
      'outlook.events.title': 'Calendar reminders', 'outlook.events.desc': 'The reminders of meetings and events that Outlook sends are listed and announced as well, under Calendar.',
      'outlook.banners.title': 'Turn off Outlook\'s banners', 'outlook.banners.desc': 'So that a mail does not show twice: in System Settings, Notifications, Microsoft Outlook, set the alert style to None. Its notifications still reach Notification Center, and so this tab.', 'outlook.banners.open': 'Open Notifications',
      'sec.outlook.privacy': 'Privacy',
      'outlook.privacy.title': 'Read, never written or sent', 'outlook.privacy.desc': 'Figghost only reads the notifications macOS keeps. Nothing is written, nothing is kept on disk and nothing leaves this Mac; the list goes when Figghost quits.',
    },
    vi: {
      'nav.outlook': 'Outlook', 'page.outlook': 'Outlook',
      'page.outlook.sub': 'Thư mới và nhắc lịch của Outlook trong một tab của notch, kèm thẻ đọc nhanh ở giữa màn hình.',
      'outlook.enable.title': 'Outlook trên notch', 'outlook.enable.desc': 'Thông báo app Outlook gửi tới được liệt kê trong tab Outlook, mới nhất ở trên; bấm vào một thư để mở thẻ đọc nhanh ở giữa màn hình. Không cần đăng nhập tài khoản: người gửi, tiêu đề và vài dòng đầu là những gì Outlook đưa vào thông báo.',
      'outlook.access.title': 'Truy cập toàn bộ ổ đĩa', 'outlook.access.ok': 'Đã cho phép: đọc được thông báo của Outlook.', 'outlook.access.no': 'Chưa cho phép: hãy bật Figghost trong danh sách, rồi thoát và mở lại app.', 'outlook.access.unknown': 'Sẽ kiểm tra khi bật tính năng: macOS cất thông báo ở chỗ chỉ ứng dụng có quyền Truy cập toàn bộ ổ đĩa mới đọc được.', 'outlook.access.missing': 'Máy này chưa cài app Outlook.', 'outlook.access.open': 'Mở danh sách',
      'sec.outlook.new': 'Khi có thư mới',
      'outlook.notch.title': 'Hiện trên notch', 'outlook.notch.desc': 'Thư mới hiện trên notch đóng trong vài giây, kèm người gửi và tiêu đề.',
      'outlook.seconds.title': 'Thời gian hiện trên notch', 'outlook.seconds.n': '{n} giây',
      'outlook.preview.title': 'Hiện nội dung', 'outlook.preview.desc': 'Tắt thì notch chỉ hiện chữ Thư mới, phù hợp khi người khác nhìn thấy màn hình.',
      'outlook.drop.title': 'Giọt nước từ notch', 'outlook.drop.desc': 'Thư mới rơi từ notch xuống giữa màn hình như một giọt nước rồi mở thành thẻ để đọc. Bấm ra chỗ khác hoặc nhấn Escape để cất đi.',
      'outlook.try': 'Thử xem',
      'outlook.events.title': 'Nhắc lịch', 'outlook.events.desc': 'Nhắc họp và sự kiện Outlook gửi tới cũng được liệt kê và báo lên, ghi là Lịch.',
      'outlook.banners.title': 'Tắt banner của Outlook', 'outlook.banners.desc': 'Để một thư không hiện hai lần: trong Cài đặt hệ thống, mục Thông báo, Microsoft Outlook, đặt kiểu thông báo là Không có. Thông báo vẫn vào Trung tâm thông báo, nên vẫn lên tab này.', 'outlook.banners.open': 'Mở mục Thông báo',
      'sec.outlook.privacy': 'Riêng tư',
      'outlook.privacy.title': 'Chỉ đọc, không ghi, không gửi đi', 'outlook.privacy.desc': 'Figghost chỉ đọc thông báo macOS lưu. Không ghi gì, không lưu gì xuống ổ đĩa và không gửi gì ra khỏi máy; danh sách mất khi Figghost thoát.',
    },
    build() {
      const seconds = () => [3, 5, 8, 12].map((n) => ({ value: n, label: t('outlook.seconds.n', { n }) }));
      const tryIt = button(t('outlook.try'), () => send({ t: 'action', name: 'outlook_try' }), { small: true });
      const banners = button(t('outlook.banners.open'), () => send({ t: 'action', name: 'outlook_banners' }), { small: true });
      const onNotch = () => on() && S.outlook.notch;
      return [
        card(toggleRow('outlook.enabled', t('outlook.enable.title'), t('outlook.enable.desc'))),
        card(accessRow()),
        sectionTitle(t('sec.outlook.new')),
        card(
          toggleRow('outlook.notch', t('outlook.notch.title'), t('outlook.notch.desc'), { when: on }),
          selectRow({ path: 'outlook.seconds', title: t('outlook.seconds.title'), desc: '', options: seconds, when: onNotch }),
          toggleRow('outlook.preview', t('outlook.preview.title'), t('outlook.preview.desc'), { when: onNotch }),
          toggleRow('outlook.drop', t('outlook.drop.title'), t('outlook.drop.desc'), { when: on }),
          row(t('outlook.try'), '', tryIt, { when: on }),
          toggleRow('outlook.events', t('outlook.events.title'), t('outlook.events.desc'), { when: on }),
          row(t('outlook.banners.title'), t('outlook.banners.desc'), banners, { when: on })),
        sectionTitle(t('sec.outlook.privacy')),
        card(infoRow(t('outlook.privacy.title'), t('outlook.privacy.desc'))),
      ];
    },
    selftest({ expect, go, sent, S }) {
      go('outlook');
      const page = document.querySelector('#scroller');
      const switches = [...page.querySelectorAll('.switch')];
      expect(switches.length === 5, 'outlook: switches for the feature, the notch, what it says, the drop and reminders: ' + switches.length);
      sent.length = 0;
      const was = S.outlook.drop;
      switches[3].click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'outlook.drop' && m.value === !was), 'outlook: the drop switch sends its setting: ' + JSON.stringify(sent));
      switches[3].click();
      sent.length = 0;
      [...page.querySelectorAll('button')].find((b) => b.textContent === t('outlook.try')).click();
      expect(sent.some((m) => m.t === 'action' && m.name === 'outlook_try'), 'outlook: the card can be tried: ' + JSON.stringify(sent));
      expect(page.querySelectorAll('select.pick').length === 1, 'outlook: a choice of the seconds on the notch');
    },
  });
})();
