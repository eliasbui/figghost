// Pomodoro: a tab of the notch that alternates work sessions and breaks, with a ring, a countdown in the closed notch and a count of the days.
(window.__plugins = window.__plugins || []).push({
  id: 'pomodoro',
  icon: 'timer',
  en: {
    'nav.pomodoro': 'Pomodoro', 'page.pomodoro': 'Pomodoro',
    'page.pomodoro.sub': 'Work in sessions with breaks between them, and watch the time in the notch.',
    'pomodoro.enable.title': 'Show the Pomodoro tab', 'pomodoro.enable.desc': 'Work sessions and breaks one after the other, a ring that fills as a stage passes, and the countdown in the closed notch while it runs. It is a tab beside Focus, which stays a plain countdown.',
    'sec.pomodoro.lengths': 'Lengths',
    'pomodoro.work.title': 'Work session', 'pomodoro.work.desc': 'How long you focus.',
    'pomodoro.short.title': 'Short break', 'pomodoro.short.desc': 'The rest between two sessions.',
    'pomodoro.long.title': 'Long break', 'pomodoro.long.desc': 'The longer rest that ends a round.',
    'pomodoro.rounds.title': 'Sessions in a round', 'pomodoro.rounds.desc': 'How many work sessions come before the long break.',
    'pomodoro.min': '{n} min', 'pomodoro.sessions': '{n} sessions',
    'sec.pomodoro.when': 'When a stage ends',
    'pomodoro.auto.title': 'Start the next stage by itself', 'pomodoro.auto.desc': 'Off waits for you to press Start after each stage.',
    'pomodoro.sound.title': 'Play a sound', 'pomodoro.sound.desc': 'A different one for the end of a session and the end of a break.',
    'pomodoro.open.title': 'Open the notch on this tab', 'pomodoro.open.desc': 'It goes away again after a few seconds if the pointer is not on it.',
    'pomodoro.closed.title': 'Show the countdown in the closed notch', 'pomodoro.closed.desc': 'While a stage runs, the time left sits beside the cutout. A click on it opens this tab.',
    'pomodoro.shortcut.title': 'Shortcut', 'pomodoro.shortcut.desc': 'Starts, pauses and resumes from any app. Press Delete while recording to remove it.',
    'target.pomodoro:toggle': 'the Pomodoro shortcut',
    'sec.pomodoro.stats': 'What was done',
    'pomodoro.stats.title': 'Sessions done', 'pomodoro.stats.desc': '{today} today. {streak} days in a row. {days} days kept.',
    'pomodoro.clear': 'Clear it all', 'pomodoro.clear.sure': 'Click again to clear',
  },
  vi: {
    'nav.pomodoro': 'Pomodoro', 'page.pomodoro': 'Pomodoro',
    'page.pomodoro.sub': 'Làm việc theo từng phiên có nghỉ xen kẽ, và xem thời gian ngay trong notch.',
    'pomodoro.enable.title': 'Hiện tab Pomodoro', 'pomodoro.enable.desc': 'Các phiên làm việc và nghỉ nối tiếp nhau, vòng tròn đầy dần theo thời gian, và đồng hồ đếm ngược trong notch đóng khi đang chạy. Đây là một tab cạnh Focus, còn Focus vẫn là đồng hồ đếm ngược đơn giản.',
    'sec.pomodoro.lengths': 'Độ dài',
    'pomodoro.work.title': 'Phiên làm việc', 'pomodoro.work.desc': 'Bạn tập trung bao lâu.',
    'pomodoro.short.title': 'Nghỉ ngắn', 'pomodoro.short.desc': 'Quãng nghỉ giữa hai phiên.',
    'pomodoro.long.title': 'Nghỉ dài', 'pomodoro.long.desc': 'Quãng nghỉ dài kết thúc một vòng.',
    'pomodoro.rounds.title': 'Số phiên trong một vòng', 'pomodoro.rounds.desc': 'Bao nhiêu phiên làm việc thì tới lần nghỉ dài.',
    'pomodoro.min': '{n} phút', 'pomodoro.sessions': '{n} phiên',
    'sec.pomodoro.when': 'Khi một giai đoạn kết thúc',
    'pomodoro.auto.title': 'Tự bắt đầu giai đoạn kế tiếp', 'pomodoro.auto.desc': 'Tắt thì chờ bạn bấm Bắt đầu sau mỗi giai đoạn.',
    'pomodoro.sound.title': 'Phát âm thanh', 'pomodoro.sound.desc': 'Khác nhau cho lúc hết phiên và lúc hết giờ nghỉ.',
    'pomodoro.open.title': 'Mở notch ở tab này', 'pomodoro.open.desc': 'Notch tự đóng lại sau vài giây nếu con trỏ không ở trên đó.',
    'pomodoro.closed.title': 'Hiện đồng hồ trong notch đóng', 'pomodoro.closed.desc': 'Khi một giai đoạn đang chạy, thời gian còn lại nằm cạnh phần khuyết. Bấm vào đó là mở tab này.',
    'pomodoro.shortcut.title': 'Phím tắt', 'pomodoro.shortcut.desc': 'Bắt đầu, tạm dừng và tiếp tục từ mọi ứng dụng. Nhấn Delete khi đang ghi phím để xóa.',
    'target.pomodoro:toggle': 'phím tắt Pomodoro',
    'sec.pomodoro.stats': 'Đã làm được gì',
    'pomodoro.stats.title': 'Số phiên đã xong', 'pomodoro.stats.desc': 'Hôm nay {today}. Liên tiếp {streak} ngày. Giữ {days} ngày.',
    'pomodoro.clear': 'Xóa hết', 'pomodoro.clear.sure': 'Bấm lần nữa để xóa',
  },
  build() {
    const on = () => S.pomodoro.enabled;
    const when = { when: on };
    const minutes = (list) => () => list.map((n) => ({ value: n, label: t('pomodoro.min', { n }) }));
    const rounds = () => [2, 3, 4, 5, 6].map((n) => ({ value: n, label: t('pomodoro.sessions', { n }) }));
    const pick = (key, list, title, desc) => selectRow({ path: 'pomodoro.' + key, title: t('pomodoro.' + title + '.title'), desc: t('pomodoro.' + title + '.desc'), options: list, when: on });
    // Clearing asks twice, so that one stray click does not throw the days away.
    let armed = false;
    let timer = 0;
    const clear = button(t('pomodoro.clear'), () => {
      if (!armed) {
        armed = true;
        clear.textContent = t('pomodoro.clear.sure');
        timer = setTimeout(() => { armed = false; clear.textContent = t('pomodoro.clear'); }, 4000);
        return;
      }
      clearTimeout(timer);
      armed = false;
      clear.textContent = t('pomodoro.clear');
      send({ t: 'action', name: 'pomodoro_clear' });
    }, { small: true, danger: true });
    const said = () => t('pomodoro.stats.desc', { today: S.pomodoro.today, streak: S.pomodoro.streak, days: S.pomodoro.days });
    const done = row(t('pomodoro.stats.title'), said(), clear, when);
    reg(() => { done.querySelector('.row-desc').textContent = said(); });
    return [
      card(toggleRow('pomodoro.enabled', t('pomodoro.enable.title'), t('pomodoro.enable.desc'))),
      sectionTitle(t('sec.pomodoro.lengths')),
      card(
        pick('work', minutes([15, 20, 25, 30, 45, 50, 60]), 'work'),
        pick('short_break', minutes([3, 5, 10, 15]), 'short'),
        pick('long_break', minutes([10, 15, 20, 30]), 'long'),
        pick('rounds', rounds, 'rounds')),
      sectionTitle(t('sec.pomodoro.when')),
      card(
        toggleRow('pomodoro.auto_start', t('pomodoro.auto.title'), t('pomodoro.auto.desc'), when),
        toggleRow('pomodoro.sound', t('pomodoro.sound.title'), t('pomodoro.sound.desc'), when),
        toggleRow('pomodoro.open_on_end', t('pomodoro.open.title'), t('pomodoro.open.desc'), when),
        toggleRow('pomodoro.closed_notch', t('pomodoro.closed.title'), t('pomodoro.closed.desc'), when),
        row(t('pomodoro.shortcut.title'), t('pomodoro.shortcut.desc'), hotkeyButton('pomodoro:toggle', { allowClear: true, label: t('pomodoro.shortcut.title') }), when)),
      sectionTitle(t('sec.pomodoro.stats')),
      card(done),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('pomodoro');
    const page = document.querySelector('#scroller');
    const switches = [...page.querySelectorAll('.switch')];
    expect(switches.length === 5, 'pomodoro: one switch for the tab and one for each of four behaviours');
    sent.length = 0;
    const was = S.pomodoro.enabled;
    switches[0].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'pomodoro.enabled' && m.value === !was), 'pomodoro: the switch sends its setting: ' + JSON.stringify(sent));
    switches[0].click();
    sent.length = 0;
    switches[1].click();
    switches[4].click();
    expect(sent.some((m) => m.path === 'pomodoro.auto_start') && sent.some((m) => m.path === 'pomodoro.closed_notch'), 'pomodoro: the behaviour switches send their own paths: ' + JSON.stringify(sent));
    switches[1].click();
    switches[4].click();

    const picks = [...page.querySelectorAll('select.pick')];
    expect(picks.length === 4, 'pomodoro: four choices of length');
    picks[0].value = '50';
    picks[0].dispatchEvent(new Event('change'));
    expect(S.pomodoro.work === 50, 'pomodoro: a length is a number of minutes');
    picks[3].value = '6';
    picks[3].dispatchEvent(new Event('change'));
    expect(S.pomodoro.rounds === 6, 'pomodoro: the sessions of a round are a number');
    picks[0].value = '25'; picks[0].dispatchEvent(new Event('change'));
    picks[3].value = '4'; picks[3].dispatchEvent(new Event('change'));

    expect(page.querySelectorAll('.hotkey').length === 1, 'pomodoro: a shortcut recorder');
    const clear = [...page.querySelectorAll('.btn')].find((b) => b.textContent === t('pomodoro.clear'));
    sent.length = 0;
    clear.click();
    expect(sent.length === 0 && clear.textContent === t('pomodoro.clear.sure'), 'pomodoro: the first click only asks');
    clear.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'pomodoro_clear'), 'pomodoro: the second click clears: ' + JSON.stringify(sent));
    expect(page.textContent.includes(S.pomodoro.days + ' '), 'pomodoro: the page says how many days are kept');
  },
});
