// Screen Time: a tab of the notch with how long each app was in front today and over the last week.
(window.__plugins = window.__plugins || []).push({
  id: 'screentime',
  icon: 'hourglass',
  en: {
    'nav.screentime': 'Screen Time', 'page.screentime': 'Screen Time',
    'page.screentime.sub': 'See which apps had your time today and this week, in a tab of the notch.',
    'screentime.enable.title': 'Count the time of each app', 'screentime.enable.desc': 'Every few seconds the app in front is given that time while you are at the Mac. Only the name of the app and the time are kept, on this Mac, for thirty days. No window title, text, screen or key is looked at, and it needs no permission. Sites are counted only when the switch below is on.',
    'sec.screentime.counting': 'Counting',
    'screentime.idle.title': 'Stop counting when I am away', 'screentime.idle.desc': 'Time stops counting once the keyboard and pointer have been still this long.',
    'screentime.idle.n': '{n} min',
    'screentime.sites.title': 'Count by site too', 'screentime.sites.desc': 'When Safari or a browser built on Chromium (Chrome, Edge, Brave, Vivaldi) is in front, the site of its front tab is given the time too, and the tab of the notch lists the sites. Only the name of the site is kept, such as github.com. Incognito windows are left out, but Safari does not say which of its windows are private, so those count. macOS asks once for each browser whether Figghost may read its tab. Firefox cannot be asked. Switching this off forgets the sites counted.',
    'sec.screentime.data': 'What was counted',
    'screentime.data.title': 'Counted so far', 'screentime.data.desc': '{days} days kept. Today: {today}.',
    'screentime.clear': 'Clear it all', 'screentime.clear.sure': 'Click again to clear', 'screentime.min': 'min', 'screentime.h': 'h',
  },
  vi: {
    'nav.screentime': 'Thời gian dùng', 'page.screentime': 'Thời gian dùng',
    'page.screentime.sub': 'Xem ứng dụng nào chiếm thời gian của bạn hôm nay và tuần này.',
    'screentime.enable.title': 'Tính thời gian của từng ứng dụng', 'screentime.enable.desc': 'Vài giây một lần, ứng dụng đang ở phía trước được cộng thời gian đó khi bạn đang dùng máy. Chỉ tên ứng dụng và thời gian được giữ lại, trên máy này, trong ba mươi ngày. Không nhìn vào tiêu đề cửa sổ, chữ, màn hình hay phím bấm, và không cần cấp quyền. Trang web chỉ được tính khi bật công tắc bên dưới.',
    'sec.screentime.counting': 'Cách tính',
    'screentime.idle.title': 'Ngừng tính khi tôi đi vắng', 'screentime.idle.desc': 'Thời gian ngừng được tính khi bàn phím và con trỏ đứng yên chừng này.',
    'screentime.idle.n': '{n} phút',
    'screentime.sites.title': 'Tính cả theo trang web', 'screentime.sites.desc': 'Khi Safari hoặc một trình duyệt dựng trên Chromium (Chrome, Edge, Brave, Vivaldi) đang ở phía trước, trang của tab đang mở cũng được cộng thời gian, và tab trên notch liệt kê các trang. Chỉ giữ tên trang, như github.com. Cửa sổ ẩn danh được bỏ qua, nhưng Safari không cho biết cửa sổ nào là riêng tư nên những cửa sổ đó vẫn được tính. macOS hỏi một lần cho mỗi trình duyệt xem Figghost có được đọc tab không. Firefox không hỏi được. Tắt công tắc này là quên hết các trang đã tính.',
    'sec.screentime.data': 'Đã tính được gì',
    'screentime.data.title': 'Đã tính cho đến giờ', 'screentime.data.desc': 'Giữ {days} ngày. Hôm nay: {today}.',
    'screentime.clear': 'Xóa hết', 'screentime.clear.sure': 'Bấm lần nữa để xóa', 'screentime.min': 'phút', 'screentime.h': 'giờ',
  },
  build() {
    const on = () => S.screentime.enabled;
    const when = { when: on };
    const idle = () => [1, 2, 5].map((n) => ({ value: n, label: t('screentime.idle.n', { n }) }));
    const said = (secs) => {
      const m = Math.floor(secs / 60);
      return m >= 60 ? Math.floor(m / 60) + ' ' + t('screentime.h') + ' ' + (m % 60) + ' ' + t('screentime.min') : m + ' ' + t('screentime.min');
    };
    // The button that throws everything away asks twice, so that one stray click does not.
    let armed = false;
    let timer = 0;
    const clear = button(t('screentime.clear'), () => {
      if (!armed) {
        armed = true;
        clear.textContent = t('screentime.clear.sure');
        timer = setTimeout(() => { armed = false; clear.textContent = t('screentime.clear'); }, 4000);
        return;
      }
      clearTimeout(timer);
      armed = false;
      clear.textContent = t('screentime.clear');
      send({ t: 'action', name: 'screentime_clear' });
    }, { small: true, danger: true });
    const counted = row(t('screentime.data.title'), t('screentime.data.desc', { days: S.screentime.days, today: said(S.screentime.today) }), clear, when);
    reg(() => { counted.querySelector('.row-desc').textContent = t('screentime.data.desc', { days: S.screentime.days, today: said(S.screentime.today) }); });
    return [
      card(toggleRow('screentime.enabled', t('screentime.enable.title'), t('screentime.enable.desc'))),
      sectionTitle(t('sec.screentime.counting')),
      card(
        selectRow({ path: 'screentime.idle_minutes', title: t('screentime.idle.title'), desc: t('screentime.idle.desc'), options: idle, when: on }),
        toggleRow('screentime.sites', t('screentime.sites.title'), t('screentime.sites.desc'), when)),
      sectionTitle(t('sec.screentime.data')),
      card(counted),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('screentime');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const master = page.querySelector('.switch');
    const was = S.screentime.enabled;
    master.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'screentime.enabled' && m.value === !was), 'screentime: the switch sends its setting: ' + JSON.stringify(sent));
    master.click();

    const sites = [...page.querySelectorAll('.switch')][1];
    const counted = S.screentime.sites;
    sent.length = 0;
    sites.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'screentime.sites' && m.value === !counted), 'screentime: counting by site has its own switch: ' + JSON.stringify(sent));
    sites.click();

    const pick = page.querySelector('select.pick');
    expect(pick.options.length === 3, 'screentime: three times to choose from');
    pick.value = '5';
    pick.dispatchEvent(new Event('change'));
    expect(S.screentime.idle_minutes === 5, 'screentime: the idle time is a number of minutes');
    pick.value = '2';
    pick.dispatchEvent(new Event('change'));

    // Clearing asks twice.
    const clear = [...page.querySelectorAll('.btn')].find((b) => b.textContent === t('screentime.clear'));
    sent.length = 0;
    clear.click();
    expect(sent.length === 0 && clear.textContent === t('screentime.clear.sure'), 'screentime: the first click only asks');
    clear.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'screentime_clear'), 'screentime: the second click clears: ' + JSON.stringify(sent));
    expect(page.textContent.includes(S.screentime.days + ' '), 'screentime: the page says how many days are kept');
  },
});
