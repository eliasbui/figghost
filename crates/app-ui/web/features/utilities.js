// Utilities: keep the Mac awake, mute the microphone for every app, lock the keyboard to clean it, snooze the notch.
(window.__plugins = window.__plugins || []).push({
  id: 'utilities',
  icon: 'slider.horizontal.3',
  en: {
    'nav.utilities': 'Utilities', 'page.utilities': 'Utilities',
    'page.utilities.sub': 'Keep the Mac awake, mute the microphone for every app at once, lock the keyboard while you wipe it, and snooze the notch.',
    'util.length.until': 'Until I stop', 'util.unit.s': 's', 'util.unit.min': 'min', 'util.unit.h': 'h',
    'util.enable.title': 'Use the utilities', 'util.enable.desc': 'Each has a place in the menu bar panel, a shortcut that works in any app, and a notice on the notch.',
    'sec.util.awake': 'Keep awake',
    'util.awake.now.title': 'Keep this Mac awake', 'util.awake.now.desc': 'Stops the Mac from sleeping on its own, as long as Figghost runs. Closing the lid still sleeps it.',
    'util.awake.start': 'Start', 'util.awake.stop': 'Stop', 'util.awake.on': 'Awake', 'util.awake.on.left': 'Awake, {left} left', 'util.awake.off': 'Off',
    'util.awake.length.title': 'Length from the shortcut', 'util.awake.length.desc': 'How long it lasts when started with the shortcut or the button. The menu bar panel offers each length.',
    'util.awake.display.title': 'Keep the screen lit too', 'util.awake.display.desc': 'Off lets the screen turn itself off while the Mac stays awake.',
    'util.awake.shortcut': 'Shortcut', 'util.awake.shortcut.desc': 'Starts or stops it. Press Delete while recording to remove it.',
    'target.utilities:awake': 'the Keep awake shortcut', 'target.utilities:mic': 'the Mute microphone shortcut', 'target.utilities:lock': 'the Lock keyboard shortcut', 'target.utilities:snooze': 'the Snooze shortcut',
    'sec.util.mic': 'Microphone',
    'util.mic.now.title': 'Mute the microphone', 'util.mic.now.desc': 'Mutes the microphone itself, so every meeting app hears nothing and none has a button to find.',
    'util.mic.mute': 'Mute', 'util.mic.unmute': 'Unmute', 'util.mic.muted': 'Muted', 'util.mic.live': 'On', 'util.mic.unknown': 'Cannot be muted',
    'sec.util.snooze': 'Snooze the notch',
    'util.snooze.now.title': 'Snooze the notch', 'util.snooze.now.desc': 'The notch stops opening when the pointer reaches the top of the screen, for when you present or play. A click on it, and anything that asks for you such as a coding agent that waits for an answer, still open it.',
    'util.snooze.start': 'Snooze', 'util.snooze.stop': 'Wake', 'util.snooze.on': 'Snoozed', 'util.snooze.on.left': 'Snoozed, {left} left', 'util.snooze.off': 'Awake',
    'util.snooze.length.title': 'Length of a snooze', 'util.snooze.length.desc': 'How long it lasts when started with the shortcut or the button. The menu bar panel offers each length.',
    'util.snooze.until': 'Until I wake it', 'util.snooze.shortcut': 'Shortcut', 'util.snooze.shortcut.desc': 'Starts or ends a snooze. Press Delete while recording to remove it.',
    'sec.util.lock': 'Keyboard lock',
    'util.lock.now.title': 'Lock the keyboard', 'util.lock.now.desc': 'Every key is ignored until the time is up, so you can wipe it without typing. Command and Escape unlock it at once. The pointer still works.',
    'util.lock.start': 'Lock', 'util.lock.stop': 'Unlock', 'util.lock.locked': 'Locked', 'util.lock.free': 'Free',
    'util.lock.seconds.title': 'Longest it stays locked', 'util.lock.seconds.desc': 'It unlocks by itself after this long.',
    'util.lock.access.title': 'Accessibility', 'util.lock.access.desc': 'Needed to hold the keys back.', 'util.lock.access.open': 'Open settings',
  },
  vi: {
    'nav.utilities': 'Tiện ích', 'page.utilities': 'Tiện ích',
    'page.utilities.sub': 'Giữ máy luôn thức, tắt tiếng micro cho mọi ứng dụng cùng lúc, khóa bàn phím khi bạn lau nó, và tạm tắt notch.',
    'util.length.until': 'Đến khi tôi tắt', 'util.unit.s': 'giây', 'util.unit.min': 'phút', 'util.unit.h': 'giờ',
    'util.enable.title': 'Dùng các tiện ích', 'util.enable.desc': 'Mỗi tiện ích có chỗ trong bảng ở thanh menu, một phím tắt dùng ở mọi ứng dụng, và một thông báo trên notch.',
    'sec.util.awake': 'Giữ thức',
    'util.awake.now.title': 'Giữ máy luôn thức', 'util.awake.now.desc': 'Không cho máy tự ngủ, chừng nào Figghost còn chạy. Đóng nắp máy vẫn làm máy ngủ.',
    'util.awake.start': 'Bắt đầu', 'util.awake.stop': 'Dừng', 'util.awake.on': 'Đang thức', 'util.awake.on.left': 'Đang thức, còn {left}', 'util.awake.off': 'Tắt',
    'util.awake.length.title': 'Thời lượng khi dùng phím tắt', 'util.awake.length.desc': 'Giữ thức bao lâu khi bắt đầu bằng phím tắt hoặc nút. Bảng ở thanh menu có đủ các mức.',
    'util.awake.display.title': 'Giữ cả màn hình sáng', 'util.awake.display.desc': 'Tắt thì màn hình có thể tự tắt trong khi máy vẫn thức.',
    'util.awake.shortcut': 'Phím tắt', 'util.awake.shortcut.desc': 'Bắt đầu hoặc dừng. Nhấn Delete khi đang ghi phím để xóa.',
    'target.utilities:awake': 'phím tắt Giữ thức', 'target.utilities:mic': 'phím tắt Tắt tiếng micro', 'target.utilities:lock': 'phím tắt Khóa bàn phím', 'target.utilities:snooze': 'phím tắt Tạm tắt notch',
    'sec.util.mic': 'Micro',
    'util.mic.now.title': 'Tắt tiếng micro', 'util.mic.now.desc': 'Tắt tiếng chính micro, nên mọi ứng dụng họp đều không nghe thấy gì và không phải tìm nút nào.',
    'util.mic.mute': 'Tắt tiếng', 'util.mic.unmute': 'Bật lại', 'util.mic.muted': 'Đã tắt tiếng', 'util.mic.live': 'Đang bật', 'util.mic.unknown': 'Không tắt tiếng được',
    'sec.util.snooze': 'Tạm tắt notch',
    'util.snooze.now.title': 'Tạm tắt notch', 'util.snooze.now.desc': 'Notch thôi mở ra khi con trỏ chạm mép trên màn hình, để bạn thuyết trình hay chơi game. Bấm vào notch, và mọi thứ cần bạn trả lời như một tác tử lập trình đang chờ, vẫn mở được nó.',
    'util.snooze.start': 'Tạm tắt', 'util.snooze.stop': 'Bật lại', 'util.snooze.on': 'Đang tạm tắt', 'util.snooze.on.left': 'Đang tạm tắt, còn {left}', 'util.snooze.off': 'Đang bật',
    'util.snooze.length.title': 'Thời gian tạm tắt', 'util.snooze.length.desc': 'Kéo dài bao lâu khi bắt đầu bằng phím tắt hoặc nút. Bảng ở thanh menu có đủ các mức.',
    'util.snooze.until': 'Đến khi tôi bật lại', 'util.snooze.shortcut': 'Phím tắt', 'util.snooze.shortcut.desc': 'Bắt đầu hoặc kết thúc tạm tắt. Nhấn Delete khi đang ghi phím để xóa.',
    'sec.util.lock': 'Khóa bàn phím',
    'util.lock.now.title': 'Khóa bàn phím', 'util.lock.now.desc': 'Mọi phím bị bỏ qua cho tới khi hết giờ, để bạn lau mà không gõ nhầm. Command và Escape mở khóa ngay. Con trỏ vẫn dùng được.',
    'util.lock.start': 'Khóa', 'util.lock.stop': 'Mở khóa', 'util.lock.locked': 'Đã khóa', 'util.lock.free': 'Bình thường',
    'util.lock.seconds.title': 'Khóa tối đa', 'util.lock.seconds.desc': 'Tự mở khóa sau chừng này thời gian.',
    'util.lock.access.title': 'Trợ năng', 'util.lock.access.desc': 'Cần để giữ các phím lại.', 'util.lock.access.open': 'Mở cài đặt',
  },
  build() {
    const on = () => S.utilities.enabled;
    const when = { when: on };
    const unit = (key) => t('util.unit.' + key);
    const left = (secs) => (secs < 60 ? secs + ' ' + unit('s') : secs < 3600 ? Math.ceil(secs / 60) + ' ' + unit('min') : Math.floor(secs / 3600) + ' ' + unit('h') + ' ' + String(Math.ceil((secs % 3600) / 60)).padStart(2, '0') + ' ' + unit('min'));

    // Keep awake
    const awakeState = h('span', { class: 'util-state' });
    const awakeBtn = button('', () => send({ t: 'action', name: 'utilities_awake' }), { small: true });
    reg(() => {
      const u = S.utilities;
      awakeBtn.textContent = u.awake ? t('util.awake.stop') : t('util.awake.start');
      awakeState.textContent = !u.awake ? t('util.awake.off') : u.awake_left != null ? t('util.awake.on.left', { left: left(u.awake_left) }) : t('util.awake.on');
    });
    const lengthOptions = () => [15, 30, 60, 120, 240, 0].map((m) => ({ value: m, label: m === 0 ? t('util.length.until') : m < 60 ? m + ' ' + unit('min') : m / 60 + ' ' + unit('h') }));

    // Microphone
    const micState = h('span', { class: 'util-state' });
    const micBtn = button('', () => send({ t: 'action', name: 'utilities_mic' }), { small: true });
    reg(() => {
      const m = S.utilities.mic_muted;
      micBtn.textContent = m ? t('util.mic.unmute') : t('util.mic.mute');
      micBtn.disabled = m === null || m === undefined;
      micState.textContent = m === null || m === undefined ? t('util.mic.unknown') : m ? t('util.mic.muted') : t('util.mic.live');
    });

    // Snooze
    const snoozeState = h('span', { class: 'util-state' });
    const snoozeBtn = button('', () => send({ t: 'action', name: 'utilities_snooze' }), { small: true });
    reg(() => {
      const u = S.utilities;
      snoozeBtn.textContent = u.snoozed ? t('util.snooze.stop') : t('util.snooze.start');
      snoozeState.textContent = !u.snoozed ? t('util.snooze.off') : u.snooze_left != null ? t('util.snooze.on.left', { left: left(u.snooze_left) }) : t('util.snooze.on');
    });
    const snoozeOptions = () => [5, 15, 30, 60, 120, 0].map((m) => ({ value: m, label: m === 0 ? t('util.snooze.until') : m < 60 ? m + ' ' + unit('min') : m / 60 + ' ' + unit('h') }));

    // Keyboard lock
    const lockState = h('span', { class: 'util-state' });
    const lockBtn = button('', () => send({ t: 'action', name: 'utilities_lock' }), { small: true });
    reg(() => {
      lockBtn.textContent = S.utilities.locked ? t('util.lock.stop') : t('util.lock.start');
      lockState.textContent = S.utilities.locked ? t('util.lock.locked') : t('util.lock.free');
    });
    return [
      card(toggleRow('utilities.enabled', t('util.enable.title'), t('util.enable.desc'))),
      sectionTitle(t('sec.util.awake')),
      card(
        row(t('util.awake.now.title'), t('util.awake.now.desc'), [awakeState, awakeBtn], when),
        selectRow({ path: 'utilities.awake_minutes', title: t('util.awake.length.title'), desc: t('util.awake.length.desc'), options: lengthOptions, when: on }),
        toggleRow('utilities.awake_display', t('util.awake.display.title'), t('util.awake.display.desc'), when),
        row(t('util.awake.shortcut'), t('util.awake.shortcut.desc'), hotkeyButton('utilities:awake', { allowClear: true, label: t('util.awake.shortcut') }), when)),
      sectionTitle(t('sec.util.mic')),
      card(
        row(t('util.mic.now.title'), t('util.mic.now.desc'), [micState, micBtn], when),
        row(t('util.awake.shortcut'), t('util.awake.shortcut.desc'), hotkeyButton('utilities:mic', { allowClear: true, label: t('util.mic.now.title') }), when)),
      sectionTitle(t('sec.util.snooze')),
      card(
        row(t('util.snooze.now.title'), t('util.snooze.now.desc'), [snoozeState, snoozeBtn], when),
        selectRow({ path: 'utilities.snooze_minutes', title: t('util.snooze.length.title'), desc: t('util.snooze.length.desc'), options: snoozeOptions, when: on }),
        row(t('util.snooze.shortcut'), t('util.snooze.shortcut.desc'), hotkeyButton('utilities:snooze', { allowClear: true, label: t('util.snooze.now.title') }), when)),
      sectionTitle(t('sec.util.lock')),
      card(
        row(t('util.lock.now.title'), t('util.lock.now.desc'), [lockState, lockBtn], when),
        sliderRow({ path: 'utilities.lock_seconds', title: t('util.lock.seconds.title'), desc: t('util.lock.seconds.desc'), min: 15, max: 300, step: 5, format: (v) => left(v) }),
        row(t('util.lock.access.title'), t('util.lock.access.desc'), [
          pill(() => (S.utilities.accessibility ? t('access.granted') : t('access.missing')), () => S.utilities.accessibility),
          button(t('util.lock.access.open'), () => send({ t: 'action', name: 'open_accessibility' }), { small: true }),
        ], when),
        row(t('util.awake.shortcut'), t('util.awake.shortcut.desc'), hotkeyButton('utilities:lock', { allowClear: true, label: t('util.lock.now.title') }), when)),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('utilities');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const master = page.querySelector('.switch');
    const was = S.utilities.enabled;
    master.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'utilities.enabled' && m.value === !was), 'utilities: the switch sends its setting: ' + JSON.stringify(sent));
    master.click();

    expect(page.querySelectorAll('.hotkey').length === 4, 'utilities: a shortcut recorder for each tool');
    const button = (label) => [...page.querySelectorAll('.btn')].find((b) => b.textContent === label);
    sent.length = 0;
    button(t('util.awake.stop')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'utilities_awake'), 'utilities: the awake button sends its action: ' + JSON.stringify(sent));
    sent.length = 0;
    button(t('util.mic.mute')).click();
    expect(sent.some((m) => m.name === 'utilities_mic'), 'utilities: the microphone button sends its action');
    sent.length = 0;
    button(t('util.snooze.start')).click();
    expect(sent.some((m) => m.name === 'utilities_snooze'), 'utilities: the snooze button sends its action');
    sent.length = 0;
    button(t('util.lock.start')).click();
    expect(sent.some((m) => m.name === 'utilities_lock'), 'utilities: the lock button sends its action');

    // The buttons follow what the app says.
    S.utilities.awake = false; S.utilities.mic_muted = true; S.utilities.locked = true; S.utilities.snoozed = true;
    runSyncs();
    expect(!!button(t('util.awake.start')) && !!button(t('util.mic.unmute')) && !!button(t('util.lock.stop')) && !!button(t('util.snooze.stop')), 'utilities: the buttons say what they would do');
    S.utilities.mic_muted = null; runSyncs();
    expect(button(t('util.mic.mute')).disabled, 'utilities: a microphone that cannot be muted has a button that is off');
    S.utilities.awake = true; S.utilities.mic_muted = false; S.utilities.locked = false; S.utilities.snoozed = false; runSyncs();
  },
});
