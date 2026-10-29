// System: a tab of the notch with the processor, the memory, the network, the disk, the heat and the fans.
(window.__plugins = window.__plugins || []).push({
  id: 'system',
  icon: 'cpu.fill',
  en: {
    'nav.system': 'System', 'page.system': 'System',
    'page.system.sub': 'See the processor, memory, network and disk at work in a tab of the notch.',
    'system.enable.title': 'Show the System tab', 'system.enable.desc': 'The tab reads the Mac only while it is open and nothing runs when it is closed. It needs no permission.',
    'sec.system.shows': 'What it shows',
    'system.processor.title': 'Processor', 'system.processor.desc': 'The load of each core, with the efficiency cores apart from the performance ones.',
    'system.memory.title': 'Memory', 'system.memory.desc': 'What apps hold, what is wired and what is compressed, with the pressure and the swap.',
    'system.network.title': 'Network', 'system.network.desc': 'Speed down and up, and the last readings drawn as a line.',
    'system.details.title': 'Graphics, disk, heat, fans and battery', 'system.details.desc': 'A row of small figures: the heat of the processor in degrees, and the speed of the fans. The fans and the battery are there only on a Mac that has them.',
    'sec.system.bar': 'Menu bar',
    'system.bar.processor.title': 'Processor load', 'system.bar.processor.desc': 'The load of all the cores together, as a percentage.',
    'system.bar.memory.title': 'Memory in use', 'system.bar.memory.desc': 'What apps, the system and compression hold, as a share of all the memory.',
    'system.bar.network.title': 'Network speed', 'system.bar.network.desc': 'Down and up together.',
    'system.bar.note.title': 'Read all the time.', 'system.bar.note': 'The figures you choose stay in the menu bar and are read as often as the tab, even while it is closed. A click on them opens the tab.',
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
    'system.details.title': 'Đồ họa, ổ đĩa, nhiệt, quạt và pin', 'system.details.desc': 'Một hàng số liệu nhỏ: nhiệt độ bộ xử lý tính bằng độ C, và tốc độ quạt. Quạt và pin chỉ có trên máy Mac có chúng.',
    'sec.system.bar': 'Thanh menu',
    'system.bar.processor.title': 'Mức tải bộ xử lý', 'system.bar.processor.desc': 'Mức tải của mọi lõi gộp lại, tính theo phần trăm.',
    'system.bar.memory.title': 'Bộ nhớ đang dùng', 'system.bar.memory.desc': 'Phần ứng dụng, hệ thống và phần nén đang giữ, so với toàn bộ bộ nhớ.',
    'system.bar.network.title': 'Tốc độ mạng', 'system.bar.network.desc': 'Tải xuống và tải lên cộng lại.',
    'system.bar.note.title': 'Đọc liên tục.', 'system.bar.note': 'Số liệu bạn chọn nằm luôn trên thanh menu và được đọc dày như tab, kể cả khi tab đóng. Bấm vào chúng để mở tab.',
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
      sectionTitle(t('sec.system.bar')),
      card(
        toggleRow('system.bar_processor', t('system.bar.processor.title'), t('system.bar.processor.desc'), when),
        toggleRow('system.bar_memory', t('system.bar.memory.title'), t('system.bar.memory.desc'), when),
        toggleRow('system.bar_network', t('system.bar.network.title'), t('system.bar.network.desc'), when),
        row(t('system.bar.note.title'), t('system.bar.note'))),
      sectionTitle(t('sec.system.how')),
      card(selectRow({ path: 'system.seconds', title: t('system.seconds.title'), desc: t('system.seconds.desc'), options: seconds, when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('system');
    const page = document.querySelector('#scroller');
    const switches = [...page.querySelectorAll('.switch')];
    expect(switches.length === 8, 'system: one switch for the tab, one for each of four cards and one for each figure of the menu bar');
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

    // Each figure of the menu bar has its own switch.
    sent.length = 0;
    switches[6].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'system.bar_memory'), 'system: a figure of the menu bar sends its own path: ' + JSON.stringify(sent));
    switches[6].click();
  },
});
