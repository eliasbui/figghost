// Day progress: a tab of the notch with how much of the day has gone and today's events laid along a line.
(window.__plugins = window.__plugins || []).push({
  id: 'dayprogress',
  icon: 'sun.horizon.fill',
  en: {
    'nav.dayprogress': 'Day progress', 'page.dayprogress': 'Day progress',
    'page.dayprogress.sub': "See how much of the day has gone, with today's events along the line, in a tab of the notch.",
    'dayprogress.enable.title': 'Show the day as a line', 'dayprogress.enable.desc': "The tab shows the share of the day that has gone, a line from the morning to bedtime with today's events laid along it, and what is on now and what comes next. It reads the same calendars as the Calendar tab and nothing else; if the calendar is not allowed yet, allow it under Notch tabs, Calendar.",
    'sec.dayprogress.line': 'The line',
    'dayprogress.wake.title': 'The day begins at', 'dayprogress.wake.desc': 'The left end of the line, where 0% is.',
    'dayprogress.bed.title': 'The day ends at', 'dayprogress.bed.desc': 'The right end of the line, where 100% is.',
    'dayprogress.am': '{h} AM', 'dayprogress.pm': '{h} PM', 'dayprogress.midnight': 'Midnight',
    'sec.dayprogress.reminders': 'Reminders',
    'dayprogress.reminders.title': 'Mark reminders on the line', 'dayprogress.reminders.desc': 'A reminder due today at a time is marked as a dot on the line, and those still to do are counted under it. It needs the Reminders access that is given under Notch tabs, Calendar.',
  },
  vi: {
    'nav.dayprogress': 'Tiến độ ngày', 'page.dayprogress': 'Tiến độ ngày',
    'page.dayprogress.sub': 'Xem ngày đã trôi qua bao nhiêu, cùng các sự kiện hôm nay trên một đường thời gian.',
    'dayprogress.enable.title': 'Hiện ngày như một đường thời gian', 'dayprogress.enable.desc': 'Tab cho thấy phần trăm của ngày đã trôi qua, một đường từ buổi sáng đến giờ đi ngủ với các sự kiện hôm nay đặt dọc theo, việc đang diễn ra và việc kế tiếp. Tab đọc đúng các lịch mà tab Lịch đọc và không đọc gì khác; nếu chưa cho phép đọc lịch, hãy cho phép ở mục Tab của notch, Lịch.',
    'sec.dayprogress.line': 'Đường thời gian',
    'dayprogress.wake.title': 'Ngày bắt đầu lúc', 'dayprogress.wake.desc': 'Đầu bên trái của đường, nơi là 0%.',
    'dayprogress.bed.title': 'Ngày kết thúc lúc', 'dayprogress.bed.desc': 'Đầu bên phải của đường, nơi là 100%.',
    'dayprogress.am': '{h24}h', 'dayprogress.pm': '{h24}h', 'dayprogress.midnight': 'Nửa đêm',
    'sec.dayprogress.reminders': 'Nhắc việc',
    'dayprogress.reminders.title': 'Đánh dấu nhắc việc trên đường', 'dayprogress.reminders.desc': 'Nhắc việc đến hạn hôm nay vào một giờ cụ thể được đánh dấu bằng một chấm trên đường, và những việc còn phải làm được đếm bên dưới. Cần quyền Nhắc nhở đã cấp ở mục Tab của notch, Lịch.',
  },
  build() {
    const on = () => S.dayprogress.enabled;
    const hour = (h) => (h === 24 ? t('dayprogress.midnight') : t(h < 12 ? 'dayprogress.am' : 'dayprogress.pm', { h: h % 12 || 12, h24: h }));
    const from = (hours) => () => hours.map((h) => ({ value: h, label: hour(h) }));
    return [
      card(toggleRow('dayprogress.enabled', t('dayprogress.enable.title'), t('dayprogress.enable.desc'))),
      sectionTitle(t('sec.dayprogress.line')),
      card(
        selectRow({ path: 'dayprogress.wake', title: t('dayprogress.wake.title'), desc: t('dayprogress.wake.desc'), options: from([5, 6, 7, 8, 9, 10]), when: on }),
        selectRow({ path: 'dayprogress.bed', title: t('dayprogress.bed.title'), desc: t('dayprogress.bed.desc'), options: from([21, 22, 23, 24]), when: on })),
      sectionTitle(t('sec.dayprogress.reminders')),
      card(toggleRow('dayprogress.reminders', t('dayprogress.reminders.title'), t('dayprogress.reminders.desc'), { when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('dayprogress');
    const page = document.querySelector('#scroller');
    const switches = [...page.querySelectorAll('.switch')];
    expect(switches.length === 2, 'dayprogress: one switch for the tab and one for the reminders');
    sent.length = 0;
    const was = S.dayprogress.enabled;
    switches[0].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'dayprogress.enabled' && m.value === !was), 'dayprogress: the switch sends its setting: ' + JSON.stringify(sent));
    switches[0].click();
    sent.length = 0;
    switches[1].click();
    expect(sent.some((m) => m.path === 'dayprogress.reminders'), 'dayprogress: the reminders switch sends its own path: ' + JSON.stringify(sent));
    switches[1].click();

    const [wake, bed] = [...page.querySelectorAll('select.pick')];
    expect(wake.options.length === 6 && bed.options.length === 4, 'dayprogress: six mornings and four bedtimes to choose from');
    expect([...bed.options].some((o) => o.value === '24' && o.textContent === t('dayprogress.midnight')), 'dayprogress: bedtime at 24 is said as midnight');
    wake.value = '6';
    wake.dispatchEvent(new Event('change'));
    expect(S.dayprogress.wake === 6, 'dayprogress: the hour is a number');
    wake.value = '7';
    wake.dispatchEvent(new Event('change'));
    bed.value = '24';
    bed.dispatchEvent(new Event('change'));
    expect(S.dayprogress.bed === 24, 'dayprogress: bedtime can be midnight');
    bed.value = '23';
    bed.dispatchEvent(new Event('change'));
  },
});
