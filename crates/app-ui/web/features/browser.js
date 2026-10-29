// Browser: the Figghost Companion extension for the browsers built on Chromium, and the bridge it talks to.
(window.__plugins = window.__plugins || []).push({
  id: 'browser',
  icon: 'safari',
  en: {
    'nav.browser': 'Browser', 'page.browser': 'Browser',
    'page.browser.sub': 'Block distracting sites while you focus, and send a video from the browser to Figghost.',
    'browser.enable.title': 'Use Figghost Companion', 'browser.enable.desc': 'An extension for Chrome, Edge, Brave, Vivaldi and Arc that talks only to Figghost on this Mac. It reads no page and asks for no access to any site.',
    'sec.browser.setup': 'Set it up',
    'browser.folder.title': '1. Load the extension', 'browser.folder.desc': 'Writes the extension into a folder and shows it in the Finder. In the browser, open the page of extensions, turn on Developer mode, choose Load unpacked and pick that folder.',
    'browser.folder.open': 'Show the folder',
    'browser.bridge.title': '2. Connect it to Figghost', 'browser.bridge.desc': 'For each browser you loaded it in: a small file in that browser\'s folder that names Figghost as the extension\'s bridge.',
    'browser.bridge.on': 'Connected', 'browser.bridge.off': 'Not connected', 'browser.bridge.add': 'Connect', 'browser.bridge.remove': 'Remove',
    'browser.none': 'No browser built on Chromium was found on this Mac.',
    'browser.remove.title': 'Remove from this Mac', 'browser.remove.desc': 'Takes the bridge away from every browser and deletes the folder of the extension, even with the feature off. The extension loaded in a browser is removed on that browser\'s page of extensions.',
    'browser.remove.all': 'Remove all',
    'browser.link.title': 'Extension', 'browser.link.on': 'Talking to Figghost', 'browser.link.off': 'Not heard from yet',
    'sec.browser.block': 'Block while you focus',
    'browser.block.title': 'Block these sites during Pomodoro work', 'browser.block.desc': 'While a work stage of Pomodoro runs, the sites below and their subdomains do not open in the browser. They open again when it ends.',
    'browser.blocking': 'Blocking now',
    'browser.sites.title': 'Sites', 'browser.sites.desc': 'One a line. An address works too; only its site is kept.',
    'sec.browser.download': 'Download',
    'browser.download.title': 'Download with Figghost', 'browser.download.desc': 'The extension\'s button and the menu of a page or a link send it to Video download. Video download must be on, with yt-dlp installed.',
  },
  vi: {
    'nav.browser': 'Trình duyệt', 'page.browser': 'Trình duyệt',
    'page.browser.sub': 'Chặn trang gây xao nhãng khi bạn tập trung, và gửi video từ trình duyệt sang Figghost.',
    'browser.enable.title': 'Dùng Figghost Companion', 'browser.enable.desc': 'Một extension cho Chrome, Edge, Brave, Vivaldi và Arc, chỉ nói chuyện với Figghost trên máy này. Nó không đọc trang nào và không xin quyền vào trang nào.',
    'sec.browser.setup': 'Cài đặt',
    'browser.folder.title': '1. Nạp extension', 'browser.folder.desc': 'Ghi extension vào một thư mục rồi mở thư mục đó trong Finder. Trong trình duyệt, mở trang tiện ích mở rộng, bật Developer mode, chọn Load unpacked rồi chọn thư mục đó.',
    'browser.folder.open': 'Mở thư mục',
    'browser.bridge.title': '2. Nối với Figghost', 'browser.bridge.desc': 'Cho mỗi trình duyệt đã nạp extension: một tệp nhỏ trong thư mục của trình duyệt đó, ghi Figghost là cầu nối của extension.',
    'browser.bridge.on': 'Đã nối', 'browser.bridge.off': 'Chưa nối', 'browser.bridge.add': 'Nối', 'browser.bridge.remove': 'Gỡ',
    'browser.none': 'Không thấy trình duyệt nào dựng trên Chromium trên máy này.',
    'browser.remove.title': 'Gỡ khỏi máy', 'browser.remove.desc': 'Xóa cầu nối khỏi mọi trình duyệt và xóa thư mục extension, kể cả khi tính năng đang tắt. Extension đã nạp trong trình duyệt thì gỡ ở trang tiện ích mở rộng của trình duyệt đó.',
    'browser.remove.all': 'Gỡ hết',
    'browser.link.title': 'Extension', 'browser.link.on': 'Đang nói chuyện với Figghost', 'browser.link.off': 'Chưa thấy',
    'sec.browser.block': 'Chặn khi tập trung',
    'browser.block.title': 'Chặn các trang này trong phiên làm việc Pomodoro', 'browser.block.desc': 'Khi một phiên làm việc của Pomodoro đang chạy, các trang dưới đây và tên miền con của chúng không mở được trong trình duyệt. Hết phiên là mở lại.',
    'browser.blocking': 'Đang chặn',
    'browser.sites.title': 'Các trang', 'browser.sites.desc': 'Mỗi dòng một trang. Dán cả địa chỉ cũng được; chỉ giữ tên trang.',
    'sec.browser.download': 'Tải video',
    'browser.download.title': 'Tải bằng Figghost', 'browser.download.desc': 'Nút của extension và menu chuột phải trên trang hoặc liên kết gửi nó sang Tải video. Cần bật Tải video và đã cài yt-dlp.',
  },
  build() {
    const on = () => S.browser.enabled;
    const when = { when: on };
    const link = pill(() => (S.browser.connected ? t('browser.link.on') : t('browser.link.off')), () => S.browser.connected);
    const found = (S.browser.browsers || []).filter((b) => b.found);
    const bridges = found.map((b) => {
      const state = pill(() => {
        const now = (S.browser.browsers || []).find((x) => x.id === b.id);
        return now && now.installed ? t('browser.bridge.on') : t('browser.bridge.off');
      }, () => !!((S.browser.browsers || []).find((x) => x.id === b.id) || {}).installed);
      const act = button('', () => {
        const now = (S.browser.browsers || []).find((x) => x.id === b.id);
        send({ t: 'action', name: (now && now.installed ? 'browser_remove:' : 'browser_install:') + b.id });
      }, { small: true });
      reg(() => {
        const now = (S.browser.browsers || []).find((x) => x.id === b.id);
        act.textContent = now && now.installed ? t('browser.bridge.remove') : t('browser.bridge.add');
      });
      return row(b.name, '', [state, act], when);
    });
    const sites = h('textarea', { class: 'browser-sites', rows: '6', 'aria-label': t('browser.sites.title'), spellcheck: 'false' });
    sites.value = S.browser.sites;
    sites.addEventListener('change', () => send({ t: 'set', path: 'browser.sites', value: sites.value }));
    reg(() => {
      if (document.activeElement !== sites) sites.value = S.browser.sites;
      sites.disabled = !on();
    });
    const blocking = h('span', { class: 'util-state' });
    reg(() => { blocking.textContent = S.browser.blocking ? t('browser.blocking') : ''; });
    return [
      card(toggleRow('browser.enabled', t('browser.enable.title'), t('browser.enable.desc'))),
      sectionTitle(t('sec.browser.setup')),
      card(
        row(t('browser.folder.title'), t('browser.folder.desc'), button(t('browser.folder.open'), () => send({ t: 'action', name: 'browser_folder' }), { small: true }), when),
        row(t('browser.bridge.title'), t('browser.bridge.desc'), null, when),
        ...(bridges.length ? bridges : [row(t('browser.none'), '', null, when)]),
        row(t('browser.link.title'), '', link, when),
        row(t('browser.remove.title'), t('browser.remove.desc'), button(t('browser.remove.all'), () => send({ t: 'action', name: 'browser_remove_all' }), { small: true, danger: true }))),
      sectionTitle(t('sec.browser.block')),
      card(
        toggleRow('browser.block', t('browser.block.title'), t('browser.block.desc'), when),
        row(t('browser.sites.title'), t('browser.sites.desc'), blocking, when),
        row('', '', sites, { block: true, ...when })),
      sectionTitle(t('sec.browser.download')),
      card(toggleRow('browser.download', t('browser.download.title'), t('browser.download.desc'), when)),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('browser');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const master = page.querySelector('.switch');
    const was = S.browser.enabled;
    master.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'browser.enabled' && m.value === !was), 'browser: the switch sends its setting: ' + JSON.stringify(sent));
    master.click();

    const button = (label) => [...page.querySelectorAll('.btn')].filter((b) => b.textContent === label);
    sent.length = 0;
    button(t('browser.folder.open'))[0].click();
    expect(sent.some((m) => m.name === 'browser_folder'), 'browser: the folder button sends its action');
    const found = S.browser.browsers.filter((b) => b.found);
    expect(button(t('browser.bridge.add')).length + button(t('browser.bridge.remove')).length === found.length, 'browser: one button for each browser found, none for one that is not there');
    sent.length = 0;
    button(t('browser.bridge.add'))[0].click();
    expect(sent.some((m) => /^browser_install:[a-z]+$/.test(m.name)), 'browser: connecting names the browser: ' + JSON.stringify(sent));
    sent.length = 0;
    button(t('browser.bridge.remove'))[0].click();
    expect(sent.some((m) => /^browser_remove:[a-z]+$/.test(m.name)), 'browser: removing names the browser');

    S.browser.enabled = false; runSyncs();
    sent.length = 0;
    button(t('browser.remove.all'))[0].click();
    expect(sent.some((m) => m.name === 'browser_remove_all'), 'browser: everything can be removed, even with the feature off');
    S.browser.enabled = true; runSyncs();

    const sites = page.querySelector('textarea.browser-sites');
    expect(sites.value === S.browser.sites, 'browser: the list shows the sites one a line');
    sent.length = 0;
    sites.value = 'youtube.com\nvnexpress.net';
    sites.dispatchEvent(new Event('change'));
    expect(sent.some((m) => m.t === 'set' && m.path === 'browser.sites' && m.value === 'youtube.com\nvnexpress.net'), 'browser: the list is sent as typed');
  },
});
