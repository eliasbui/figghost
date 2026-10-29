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
    'system.bar.network.title': 'Network speed', 'system.bar.network.desc': 'Down and up together as text, or up over down.',
    'system.bar.gpu.title': 'Graphics load', 'system.bar.gpu.desc': 'How busy the graphics processor is, as a percentage.',
    'system.bar.heat.title': 'Processor heat', 'system.bar.heat.desc': 'In degrees, on a Mac that reports it.',
    'system.bar.disk.title': 'Startup disk', 'system.bar.disk.desc': 'The room left on the startup disk.',
    'system.kind.off': 'Off', 'system.kind.text': 'Text', 'system.kind.stacked': 'Label over value', 'system.kind.rates': 'Up over down',
    'system.kind.graph': 'Graph', 'system.kind.cores': 'Each core',
    'system.bar.combined.title': 'One item for all', 'system.bar.combined.desc': 'Off, each figure is an item of its own that Command-drag moves.',
    'system.bar.color.title': 'Graphs in colour', 'system.bar.color.desc': 'Lines and cores in the accent colour, a core above 85% in red. Off, they take the colour of the menu bar.',
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
    'system.bar.network.title': 'Tốc độ mạng', 'system.bar.network.desc': 'Tải xuống và tải lên cộng lại khi là chữ, hoặc tải lên trên tải xuống.',
    'system.bar.gpu.title': 'Mức tải đồ họa', 'system.bar.gpu.desc': 'Bộ xử lý đồ họa đang bận đến đâu, tính theo phần trăm.',
    'system.bar.heat.title': 'Nhiệt độ bộ xử lý', 'system.bar.heat.desc': 'Tính bằng độ C, trên máy Mac có báo.',
    'system.bar.disk.title': 'Ổ khởi động', 'system.bar.disk.desc': 'Dung lượng còn trống trên ổ khởi động.',
    'system.kind.off': 'Tắt', 'system.kind.text': 'Chữ', 'system.kind.stacked': 'Nhãn trên số', 'system.kind.rates': 'Lên trên, xuống dưới',
    'system.kind.graph': 'Đồ thị', 'system.kind.cores': 'Từng lõi',
    'system.bar.combined.title': 'Gộp thành một mục', 'system.bar.combined.desc': 'Tắt thì mỗi số liệu là một mục riêng, giữ Command và kéo để đổi chỗ.',
    'system.bar.color.title': 'Đồ thị có màu', 'system.bar.color.desc': 'Đường và lõi theo màu nhấn, lõi quá 85% màu đỏ. Tắt thì cùng màu chữ của thanh menu.',
    'system.bar.note.title': 'Đọc liên tục.', 'system.bar.note': 'Số liệu bạn chọn nằm luôn trên thanh menu và được đọc dày như tab, kể cả khi tab đóng. Bấm vào chúng để mở tab.',
    'sec.system.how': 'Lần đọc',
    'system.seconds.title': 'Đọc bao lâu một lần', 'system.seconds.desc': 'Đọc dày thì bắt kịp những đợt tăng vọt, đọc thưa thì máy ít việc hơn.',
    'system.every.one': 'Mỗi giây', 'system.every.many': 'Mỗi {n} giây',
  },
  build() {
    const on = () => S.system.enabled;
    const when = { when: on };
    const seconds = () => [1, 2, 5].map((n) => ({ value: n, label: n === 1 ? t('system.every.one') : t('system.every.many', { n }) }));
    // One choice per figure: off, or how it is drawn. Off is its switch and the rest its kind, so a figure that was on
    // before kinds existed stays on.
    const KINDS = { processor: ['text', 'stacked', 'graph', 'cores'], network: ['text', 'stacked', 'graph'] };
    const figure = (id) => {
      const flag = 'bar_' + id, kind = 'bar_' + id + '_kind', title = t('system.bar.' + id + '.title');
      const label = (k) => t('system.kind.' + (id === 'network' && k === 'stacked' ? 'rates' : k));
      const el = h('select', { class: 'pick', 'aria-label': title });
      el.addEventListener('change', () => {
        if (el.value === 'off') { change('system.' + flag, false); return; }
        change('system.' + kind, el.value);
        change('system.' + flag, true);
      });
      reg(() => {
        if (el.dataset.key !== lang) {
          el.dataset.key = lang;
          el.replaceChildren(...['off', ...(KINDS[id] || ['text', 'stacked', 'graph'])].map((k) => h('option', { value: k, text: label(k) })));
        }
        el.value = S.system[flag] ? S.system[kind] : 'off';
      });
      return row(title, t('system.bar.' + id + '.desc'), el, when);
    };
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
        ...['processor', 'memory', 'network', 'gpu', 'heat', 'disk'].map(figure),
        row(t('system.bar.note.title'), t('system.bar.note'))),
      card(
        toggleRow('system.bar_combined', t('system.bar.combined.title'), t('system.bar.combined.desc'), when),
        toggleRow('system.bar_color', t('system.bar.color.title'), t('system.bar.color.desc'), when)),
      sectionTitle(t('sec.system.how')),
      card(selectRow({ path: 'system.seconds', title: t('system.seconds.title'), desc: t('system.seconds.desc'), options: seconds, when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('system');
    const page = document.querySelector('#scroller');
    const switches = [...page.querySelectorAll('.switch')];
    expect(switches.length === 7, 'system: one switch for the tab, one for each of four cards, one to combine the figures of the menu bar and one for colour');
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
    const picks = [...page.querySelectorAll('select.pick')];
    expect(picks.length === 7, 'system: a choice for each of six figures of the menu bar and one for the interval');
    const pick = picks[6];
    expect(pick.options.length === 3, 'system: three intervals to choose from');
    pick.value = '5';
    pick.dispatchEvent(new Event('change'));
    expect(S.system.seconds === 5, 'system: the interval is a number of seconds');
    pick.value = '1';
    pick.dispatchEvent(new Event('change'));

    // Each figure of the menu bar is off or drawn one way; only the processor can be drawn core by core.
    const [cpu, memory, network] = picks;
    expect(cpu.options.length === 5 && memory.options.length === 4, 'system: off and three kinds, and the cores for the processor');
    expect([...network.options].some((o) => o.value === 'stacked' && o.text === t('system.kind.rates')), 'system: the network is up over down');
    sent.length = 0;
    memory.value = 'graph';
    memory.dispatchEvent(new Event('change'));
    expect(S.system.bar_memory === true && S.system.bar_memory_kind === 'graph', 'system: a kind turns the figure on');
    memory.value = 'off';
    memory.dispatchEvent(new Event('change'));
    expect(S.system.bar_memory === false && S.system.bar_memory_kind === 'graph', 'system: off keeps the kind for next time');
    expect(memory.value === 'off', 'system: the choice shows off');
    sent.length = 0;
    switches[5].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'system.bar_combined'), 'system: combining sends its own path: ' + JSON.stringify(sent));
    switches[5].click();
  },
});
