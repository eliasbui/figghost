// Devices: a tab of the notch with the charge of the Bluetooth devices that are connected.
(window.__plugins = window.__plugins || []).push({
  id: 'devices',
  icon: 'airpodspro',
  en: {
    'nav.devices': 'Devices', 'page.devices': 'Devices',
    'page.devices.sub': 'See the charge of your Bluetooth mouse, keyboard and headphones in a tab of the notch.',
    'devices.enable.title': 'Show the charge of connected devices', 'devices.enable.desc': 'The tab lists the Bluetooth devices that are connected, each with the charge it reports, and marks one that is running low. The system is asked when the tab opens and every half minute while it shows, and never otherwise. A device that does not report its charge to the Mac is listed without one. It needs no permission and nothing is stored.',
    'sec.devices.low': 'Low charge',
    'devices.low.title': 'Call a device low at', 'devices.low.desc': 'A device at or under this charge gets a warning colour and the word Low. A pair of earbuds counts each bud, not the case.',
    'devices.low.n': '{n}%',
  },
  vi: {
    'nav.devices': 'Thiết bị', 'page.devices': 'Thiết bị',
    'page.devices.sub': 'Xem mức pin của chuột, bàn phím và tai nghe Bluetooth ngay trong một tab của notch.',
    'devices.enable.title': 'Hiện mức pin của thiết bị đang kết nối', 'devices.enable.desc': 'Tab liệt kê các thiết bị Bluetooth đang kết nối, mỗi thiết bị kèm mức pin nó báo, và đánh dấu thiết bị sắp hết pin. Máy chỉ hỏi hệ thống khi tab mở và nửa phút một lần khi tab còn hiện, ngoài ra không hỏi. Thiết bị nào không báo mức pin cho máy sẽ được liệt kê mà không có số. Không cần cấp quyền và không lưu gì.',
    'sec.devices.low': 'Pin yếu',
    'devices.low.title': 'Coi là yếu khi còn', 'devices.low.desc': 'Thiết bị có mức pin bằng hoặc dưới mức này được tô màu cảnh báo và ghi chữ Pin yếu. Tai nghe nhét tai tính từng bên tai, không tính hộp sạc.',
    'devices.low.n': '{n}%',
  },
  build() {
    const on = () => S.devices.enabled;
    const levels = () => [10, 15, 20, 30].map((n) => ({ value: n, label: t('devices.low.n', { n }) }));
    return [
      card(toggleRow('devices.enabled', t('devices.enable.title'), t('devices.enable.desc'))),
      sectionTitle(t('sec.devices.low')),
      card(selectRow({ path: 'devices.low_at', title: t('devices.low.title'), desc: t('devices.low.desc'), options: levels, when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('devices');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const was = S.devices.enabled;
    page.querySelector('.switch').click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'devices.enabled' && m.value === !was), 'devices: the switch sends its setting: ' + JSON.stringify(sent));
    page.querySelector('.switch').click();
    const pick = page.querySelector('select.pick');
    expect(pick.options.length === 4, 'devices: four levels to choose from');
    pick.value = '30';
    pick.dispatchEvent(new Event('change'));
    expect(S.devices.low_at === 30, 'devices: the level is a number');
    pick.value = '20';
    pick.dispatchEvent(new Event('change'));
  },
});
