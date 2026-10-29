// Breaks: a short break for the eyes after a while at the Mac, with reminders to blink and to sit up.
(window.__plugins = window.__plugins || []).push({
  id: 'breaks',
  icon: 'eye',
  en: {
    'nav.breaks': 'Breaks', 'page.breaks': 'Breaks',
    'page.breaks.sub': 'Rest your eyes for a moment after a while at the Mac, and be reminded to blink and to sit up.',
    'breaks.enable.title': 'Remind me to rest my eyes', 'breaks.enable.desc': 'Only time at the Mac counts. A break is announced on the notch ten seconds before it starts.',
    'breaks.next.title': 'Next break', 'breaks.next.in': 'In {left}', 'breaks.next.now': 'Resting now', 'breaks.next.off': 'Off',
    'breaks.now': 'Rest now', 'breaks.end': 'End the break',
    'breaks.min': 'min', 'breaks.s': 's',
    'sec.breaks.rhythm': 'Rhythm',
    'breaks.work.title': 'A break every', 'breaks.work.desc': 'Twenty minutes is the 20-20-20 rule: every twenty minutes, look twenty feet away for twenty seconds.',
    'breaks.length.title': 'Each break lasts', 'breaks.length.desc': 'Five minutes is time to stand up and walk a little.',
    'breaks.cover.title': 'Cover the screens', 'breaks.cover.desc': 'Blurs every display with a card to skip the break or have it five minutes later. Off, the break shows on the notch only.',
    'breaks.sound.title': 'Sound', 'breaks.sound.desc': 'A soft sound when a break starts and ends.',
    'breaks.shortcut.title': 'Shortcut', 'breaks.shortcut.desc': 'Starts a break now, or ends the one that runs. Press Delete while recording to remove it.',
    'target.breaks:now': 'the Rest now shortcut',
    'sec.breaks.nudges': 'Small reminders',
    'breaks.blink.title': 'Blink', 'breaks.blink.desc': 'A notice on the notch, for eyes that stare at a screen and forget to blink.',
    'breaks.posture.title': 'Sit up', 'breaks.posture.desc': 'A notice on the notch to sit up straight and let the shoulders drop.',
    'breaks.never': 'Never', 'breaks.every': 'Every {n} min',
    'sec.breaks.hold': 'Wait while',
    'breaks.meetings.title': 'In a meeting', 'breaks.meetings.desc': 'A camera or a microphone is in use. The break comes as soon as the meeting ends.',
    'breaks.fullscreen.title': 'An app is full screen', 'breaks.fullscreen.desc': 'A film, a game or a presentation fills the screen.',
    'breaks.note.title': 'Away counts as a break.', 'breaks.note': 'Keyboard and pointer still for five minutes, or for a whole break if that is longer, start the count again. Reading or watching without touching still counts, since the eyes stay on the screen.',
  },
  vi: {
    'nav.breaks': 'Nghỉ mắt', 'page.breaks': 'Nghỉ mắt',
    'page.breaks.sub': 'Cho mắt nghỉ một lát sau một quãng dùng máy, và được nhắc chớp mắt, ngồi thẳng.',
    'breaks.enable.title': 'Nhắc tôi cho mắt nghỉ', 'breaks.enable.desc': 'Chỉ tính thời gian bạn đang dùng máy. Notch báo trước mười giây khi sắp đến giờ nghỉ.',
    'breaks.next.title': 'Lần nghỉ tới', 'breaks.next.in': 'Sau {left}', 'breaks.next.now': 'Đang nghỉ', 'breaks.next.off': 'Tắt',
    'breaks.now': 'Nghỉ ngay', 'breaks.end': 'Kết thúc nghỉ',
    'breaks.min': 'phút', 'breaks.s': 'giây',
    'sec.breaks.rhythm': 'Nhịp nghỉ',
    'breaks.work.title': 'Nghỉ một lần sau mỗi', 'breaks.work.desc': 'Hai mươi phút là quy tắc 20-20-20: cứ hai mươi phút, nhìn xa sáu mét trong hai mươi giây.',
    'breaks.length.title': 'Mỗi lần nghỉ', 'breaks.length.desc': 'Năm phút là đủ để đứng dậy đi lại một chút.',
    'breaks.cover.title': 'Che màn hình', 'breaks.cover.desc': 'Làm mờ mọi màn hình, ở giữa có thẻ để bỏ qua hoặc để năm phút nữa. Tắt thì chỉ báo trên notch.',
    'breaks.sound.title': 'Âm thanh', 'breaks.sound.desc': 'Một tiếng nhẹ khi bắt đầu và khi hết giờ nghỉ.',
    'breaks.shortcut.title': 'Phím tắt', 'breaks.shortcut.desc': 'Bắt đầu nghỉ ngay, hoặc kết thúc lần nghỉ đang chạy. Nhấn Delete khi đang ghi phím để xóa.',
    'target.breaks:now': 'phím tắt Nghỉ ngay',
    'sec.breaks.nudges': 'Nhắc nhỏ',
    'breaks.blink.title': 'Chớp mắt', 'breaks.blink.desc': 'Một thông báo trên notch, cho đôi mắt nhìn màn hình mà quên chớp.',
    'breaks.posture.title': 'Ngồi thẳng', 'breaks.posture.desc': 'Một thông báo trên notch để ngồi thẳng lưng và thả lỏng vai.',
    'breaks.never': 'Không nhắc', 'breaks.every': 'Mỗi {n} phút',
    'sec.breaks.hold': 'Chờ khi',
    'breaks.meetings.title': 'Đang họp', 'breaks.meetings.desc': 'Camera hoặc micro đang được dùng. Họp xong là đến giờ nghỉ ngay.',
    'breaks.fullscreen.title': 'Có ứng dụng toàn màn hình', 'breaks.fullscreen.desc': 'Phim, trò chơi hay bài trình chiếu đang chiếm cả màn hình.',
    'breaks.note.title': 'Rời máy cũng là nghỉ.', 'breaks.note': 'Bàn phím và con trỏ đứng yên năm phút, hoặc cả một lần nghỉ nếu lâu hơn, thì bắt đầu đếm lại. Đọc hay xem mà không chạm vẫn được tính, vì mắt vẫn nhìn màn hình.',
  },
  build() {
    const on = () => S.breaks.enabled;
    const when = { when: on };
    const said = (secs) => (secs < 60 ? secs + ' ' + t('breaks.s') : Math.ceil(secs / 60) + ' ' + t('breaks.min'));
    const state = h('span', { class: 'util-state' });
    const now = button('', () => send({ t: 'action', name: 'breaks_now' }), { small: true });
    reg(() => {
      const b = S.breaks;
      state.textContent = !b.enabled ? t('breaks.next.off') : b.breaking ? t('breaks.next.now') : b.until != null ? t('breaks.next.in', { left: said(b.until) }) : '';
      now.textContent = b.breaking ? t('breaks.end') : t('breaks.now');
    });
    const work = () => [20, 30, 45, 60].map((m) => ({ value: m, label: m + ' ' + t('breaks.min') }));
    const length = () => [20, 60, 300].map((s) => ({ value: s, label: said(s) }));
    const nudge = () => [0, 10, 20, 30].map((m) => ({ value: m, label: m === 0 ? t('breaks.never') : t('breaks.every', { n: m }) }));
    return [
      card(toggleRow('breaks.enabled', t('breaks.enable.title'), t('breaks.enable.desc'))),
      card(row(t('breaks.next.title'), '', [state, now], when)),
      sectionTitle(t('sec.breaks.rhythm')),
      card(
        selectRow({ path: 'breaks.work_minutes', title: t('breaks.work.title'), desc: t('breaks.work.desc'), options: work, when: on }),
        selectRow({ path: 'breaks.break_seconds', title: t('breaks.length.title'), desc: t('breaks.length.desc'), options: length, when: on }),
        toggleRow('breaks.cover', t('breaks.cover.title'), t('breaks.cover.desc'), when),
        toggleRow('breaks.sound', t('breaks.sound.title'), t('breaks.sound.desc'), when),
        row(t('breaks.shortcut.title'), t('breaks.shortcut.desc'), hotkeyButton('breaks:now', { allowClear: true, label: t('breaks.now') }), when)),
      sectionTitle(t('sec.breaks.nudges')),
      card(
        selectRow({ path: 'breaks.blink_minutes', title: t('breaks.blink.title'), desc: t('breaks.blink.desc'), options: nudge, when: on }),
        selectRow({ path: 'breaks.posture_minutes', title: t('breaks.posture.title'), desc: t('breaks.posture.desc'), options: nudge, when: on })),
      sectionTitle(t('sec.breaks.hold')),
      card(
        toggleRow('breaks.hold_meetings', t('breaks.meetings.title'), t('breaks.meetings.desc'), when),
        toggleRow('breaks.hold_full_screen', t('breaks.fullscreen.title'), t('breaks.fullscreen.desc'), when),
        row(t('breaks.note.title'), t('breaks.note'))),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('breaks');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const master = page.querySelector('.switch');
    const was = S.breaks.enabled;
    master.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'breaks.enabled' && m.value === !was), 'breaks: the switch sends its setting: ' + JSON.stringify(sent));
    master.click();

    const picks = [...page.querySelectorAll('select.pick')];
    expect(picks.length === 4, 'breaks: four lengths to choose');
    picks[0].value = '45';
    picks[0].dispatchEvent(new Event('change'));
    expect(S.breaks.work_minutes === 45, 'breaks: the time between breaks is a number of minutes');
    picks[0].value = '20';
    picks[0].dispatchEvent(new Event('change'));
    expect([...picks[2].options].some((o) => o.value === '0' && o.textContent === t('breaks.never')), 'breaks: a reminder can be never');

    expect(page.querySelectorAll('.hotkey').length === 1, 'breaks: one shortcut recorder');
    const button = (label) => [...page.querySelectorAll('.btn')].find((b) => b.textContent === label);
    sent.length = 0;
    button(t('breaks.now')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'breaks_now'), 'breaks: the button sends its action');
    S.breaks.breaking = true; runSyncs();
    expect(!!button(t('breaks.end')) && page.textContent.includes(t('breaks.next.now')), 'breaks: a break that runs can be ended');
    S.breaks.breaking = false; runSyncs();
  },
});
