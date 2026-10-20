// System: a tab of the notch with the processor, the memory, the network, the disk and the heat.
(window.__plugins = window.__plugins || []).push({
  id: 'system',
  icon: 'cpu',
  en: {
    'nav.system': 'System', 'page.system': 'System',
    'page.system.sub': 'See the processor, memory, network and disk at work in a tab of the notch.',
    'system.enable.title': 'Show the System tab', 'system.enable.desc': 'The tab reads the Mac only while it is open and nothing runs when it is closed. It needs no permission.',
    'sec.system.shows': 'What it shows',
    'system.processor.title': 'Processor', 'system.processor.desc': 'The load of each core, with the efficiency cores apart from the performance ones.',
    'system.memory.title': 'Memory', 'system.memory.desc': 'What apps hold, what is wired and what is compressed, with the pressure and the swap.',
    'system.network.title': 'Network', 'system.network.desc': 'Speed down and up, and the last readings drawn as a line.',
    'system.details.title': 'Graphics, disk, heat and battery', 'system.details.desc': 'A row of small figures. The battery is there only on a Mac that has one.',
    'sec.system.how': 'Readings',
    'system.seconds.title': 'How often', 'system.seconds.desc': 'A short wait follows a spike closely and a long one reads less.',
    'system.every.one': 'Every second', 'system.every.many': 'Every {n} seconds',
  },
  vi: {
    'nav.system': 'Hệ thống', 'page.system': 'Hệ thống',
    'page.system.sub': 'Xem bộ xử lý, bộ nhớ, mạng và ổ đĩa đang làm gì, ngay trong một tab của notch.',
    'system.enable.title': 'Hiện tab Hệ thống', 'system.enable.desc': 'Tab chỉ đọc máy khi đang mở, đóng lại là không chạy gì. Không cần cấp quyền.',
    'sec.system.shows': 'Hiển thị gì',
    'system.processor.title': 'Bộ xử lý', 'system.processor.desc': 'Mức tải từng lõi, tách lõi tiết kiệm khỏi lõi hiệu năng.',
    'system.memory.title': 'Bộ nhớ', 'system.memory.desc': 'Phần ứng dụng giữ, phần hệ thống ghim và phần bị nén, cùng áp lực và swap.',
    'system.network.title': 'Mạng', 'system.network.desc': 'Tốc độ tải xuống và tải lên, và các lần đọc gần nhất vẽ thành một đường.',
    'system.details.title': 'Đồ họa, ổ đĩa, nhiệt và pin', 'system.details.desc': 'Một hàng số liệu nhỏ. Pin chỉ có trên máy Mac có pin.',
    'sec.system.how': 'Lần đọc',
    'system.seconds.title': 'Đọc bao lâu một lần', 'system.seconds.desc': 'Đọc dày thì bắt kịp những đợt tăng vọt, đọc thưa thì máy ít việc hơn.',
    'system.every.one': 'Mỗi giây', 'system.every.many': 'Mỗi {n} giây',
  },
  build() {
    const on = () => S.system.enabled;
    const when = { when: on };
    const seconds = () => [1, 2, 5].map((n) => ({ value: n, label: n === 1 ? t('system.every.one') : t('system.every.many', { n }) }));
    return [
      card(toggleRow('system.enabled', t('system.enable.title'), t('system.enable.desc'))),
      sectionTitle(t('sec.system.shows')),
      card(
        toggleRow('system.processor', t('system.processor.title'), t('system.processor.desc'), when),
        toggleRow('system.memory', t('system.memory.title'), t('system.memory.desc'), when),
        toggleRow('system.network', t('system.network.title'), t('system.network.desc'), when),
        toggleRow('system.details', t('system.details.title'), t('system.details.desc'), when)),
      sectionTitle(t('sec.system.how')),
      card(selectRow({ path: 'system.seconds', title: t('system.seconds.title'), desc: t('system.seconds.desc'), options: seconds, when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('system');
    const page = document.querySelector('#scroller');
    const switches = [...page.querySelectorAll('.switch')];
    expect(switches.length === 5, 'system: one switch for the tab and one for each of four cards');
    sent.length = 0;
    const was = S.system.enabled;
    switches[0].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'system.enabled' && m.value === !was), 'system: the switch sends its setting: ' + JSON.stringify(sent));
    switches[0].click();

    // Each card has its own switch, and the interval is one of three choices that is sent as a number.
    sent.length = 0;
    switches[2].click();
    expect(sent.some((m) => m.path === 'system.memory'), 'system: a card switch sends its own path: ' + JSON.stringify(sent));
    switches[2].click();
    const pick = page.querySelector('select.pick');
    expect(pick.options.length === 3, 'system: three intervals to choose from');
    pick.value = '5';
    pick.dispatchEvent(new Event('change'));
    expect(S.system.seconds === 5, 'system: the interval is a number of seconds');
    pick.value = '1';
    pick.dispatchEvent(new Event('change'));
  },
});
