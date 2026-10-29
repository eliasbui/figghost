// Dictation: speak into any text box. A shortcut brings a microphone to the middle of the screen, and the words go in.
(window.__plugins = window.__plugins || []).push({
  id: 'dictation',
  icon: 'mic',
  en: {
    'nav.dictation': 'Dictation', 'page.dictation': 'Dictation',
    'page.dictation.sub': 'Put the caret in a text box, press the shortcut and speak. The words go in when you stop.',
    'dictation.enable.title': 'Speak instead of typing', 'dictation.enable.desc': 'A microphone comes to the middle of the screen and shows the words as they are understood. Return puts them in, Escape throws them away, the arrows change the language.',
    'dictation.shortcut.title': 'Shortcut', 'dictation.shortcut.desc': 'Press it again to put the words in now.', 'dictation.start': 'Start dictation',
    'sec.dictation.language': 'Language',
    'dictation.language.title': 'Language spoken', 'dictation.language.desc': 'Each language has its own model, the one macOS keeps.',
    'dictation.language.auto': 'As the app', 'dictation.language.vi': 'Vietnamese', 'dictation.language.en': 'English',
    'dictation.model.vi': 'Vietnamese', 'dictation.model.en': 'English',
    'dictation.model.local': 'Recognized on this Mac', 'dictation.model.server': 'Needs Apple\'s servers', 'dictation.model.none': 'Not available on this Mac',
    'dictation.on_device.title': 'Only on this Mac', 'dictation.on_device.desc': 'The audio never leaves the Mac. Off, a language this Mac cannot recognize by itself is sent to Apple to be recognized.',
    'sec.dictation.listening': 'Listening',
    'dictation.silence_seconds.title': 'Put the words in after a silence of', 'dictation.silence_seconds.desc': 'Counted from the last word. With Never, the words go in only with Return or the shortcut.',
    'dictation.silence_seconds.0': 'Never', 'dictation.silence_seconds.2': '2 s', 'dictation.silence_seconds.3': '3 s', 'dictation.silence_seconds.5': '5 s',
    'dictation.sound.title': 'Sounds', 'dictation.sound.desc': 'A soft sound when the microphone opens and when the words go in.',
    'sec.dictation.access': 'Access',
    'dictation.speech.title': 'Speech Recognition', 'dictation.speech.desc': 'To turn the voice into words.',
    'dictation.microphone.title': 'Microphone', 'dictation.microphone.desc': 'Only while the panel is open.',
    'dictation.accessibility.title': 'Accessibility', 'dictation.accessibility.desc': 'To put the words into the text box. Without it they are left on the clipboard.',
    'dictation.access.yes': 'Allowed', 'dictation.access.no': 'Not allowed', 'dictation.access.ask': 'Not asked yet',
    'dictation.allow': 'Allow...', 'dictation.open': 'Open System Settings',
  },
  vi: {
    'nav.dictation': 'Nói thành chữ', 'page.dictation': 'Nói thành chữ',
    'page.dictation.sub': 'Đặt con trỏ vào một ô nhập, nhấn phím tắt và nói. Chữ được chèn vào khi bạn ngừng nói.',
    'dictation.enable.title': 'Nói thay cho gõ', 'dictation.enable.desc': 'Một micrô hiện ở giữa màn hình và cho thấy chữ ngay khi được nhận ra. Return để chèn, Esc để bỏ, phím mũi tên để đổi ngôn ngữ.',
    'dictation.shortcut.title': 'Phím tắt', 'dictation.shortcut.desc': 'Nhấn lần nữa để chèn chữ ngay.', 'dictation.start': 'Bắt đầu nói',
    'sec.dictation.language': 'Ngôn ngữ',
    'dictation.language.title': 'Ngôn ngữ nói', 'dictation.language.desc': 'Mỗi ngôn ngữ có một mô hình riêng, là mô hình macOS có sẵn.',
    'dictation.language.auto': 'Theo ứng dụng', 'dictation.language.vi': 'Tiếng Việt', 'dictation.language.en': 'Tiếng Anh',
    'dictation.model.vi': 'Tiếng Việt', 'dictation.model.en': 'Tiếng Anh',
    'dictation.model.local': 'Nhận dạng ngay trên máy này', 'dictation.model.server': 'Cần máy chủ của Apple', 'dictation.model.none': 'Máy này chưa có',
    'dictation.on_device.title': 'Chỉ trên máy này', 'dictation.on_device.desc': 'Âm thanh không bao giờ rời khỏi máy. Nếu tắt, ngôn ngữ nào máy chưa tự nhận dạng được sẽ được gửi tới Apple để nhận dạng.',
    'sec.dictation.listening': 'Khi nghe',
    'dictation.silence_seconds.title': 'Tự chèn chữ sau khi im lặng', 'dictation.silence_seconds.desc': 'Tính từ chữ cuối cùng. Chọn Không bao giờ thì chữ chỉ được chèn khi nhấn Return hoặc phím tắt.',
    'dictation.silence_seconds.0': 'Không bao giờ', 'dictation.silence_seconds.2': '2 giây', 'dictation.silence_seconds.3': '3 giây', 'dictation.silence_seconds.5': '5 giây',
    'dictation.sound.title': 'Âm thanh', 'dictation.sound.desc': 'Một tiếng nhỏ khi micrô mở và khi chữ được chèn.',
    'sec.dictation.access': 'Quyền truy cập',
    'dictation.speech.title': 'Nhận dạng giọng nói', 'dictation.speech.desc': 'Để chuyển giọng nói thành chữ.',
    'dictation.microphone.title': 'Micrô', 'dictation.microphone.desc': 'Chỉ trong lúc bảng nghe đang mở.',
    'dictation.accessibility.title': 'Trợ năng', 'dictation.accessibility.desc': 'Để chèn chữ vào ô nhập. Không có quyền này, chữ được để lại trong clipboard.',
    'dictation.access.yes': 'Đã cho phép', 'dictation.access.no': 'Chưa cho phép', 'dictation.access.ask': 'Chưa hỏi',
    'dictation.allow': 'Cho phép...', 'dictation.open': 'Mở Cài đặt hệ thống',
  },
  build() {
    const on = () => S.dictation.enabled;
    const when = { when: on };
    const act = (name) => send({ t: 'action', name });

    const model = (code) => {
      const status = h('span', { class: 'row-desc' });
      reg(() => {
        const m = S.dictation.models[code];
        status.textContent = t('dictation.model.' + (m.on_device ? 'local' : m.available ? 'server' : 'none'));
      });
      return row(t('dictation.model.' + code), null, status, when);
    };

    // One row of access: what it is now, and the one button that moves it on.
    const access = (key, state, ask, open) => {
      const status = h('span', { class: 'row-desc dictation-status' });
      const allow = button(t('dictation.allow'), () => act(ask), { small: true });
      const settings = button(t('dictation.open'), () => act(open), { small: true });
      reg(() => {
        const now = state();
        status.textContent = t('dictation.access.' + now);
        allow.style.display = now === 'ask' && ask ? '' : 'none';
        settings.style.display = now === 'no' || (now === 'ask' && !ask) ? '' : 'none';
      });
      return row(t('dictation.' + key + '.title'), t('dictation.' + key + '.desc'), h('div', { class: 'dictation-access' }, status, allow, settings), when);
    };
    const speech = () => ({ allowed: 'yes', denied: 'no' })[S.dictation.speech] || 'ask';
    const microphone = () => (S.dictation.microphone ? 'yes' : S.dictation.speech === 'ask' ? 'ask' : 'no');

    return [
      card(toggleRow('dictation.enabled', t('dictation.enable.title'), t('dictation.enable.desc'))),
      card(row(t('dictation.shortcut.title'), t('dictation.shortcut.desc'), hotkeyButton('dictation', { allowClear: true, label: t('dictation.start') }), when)),
      sectionTitle(t('sec.dictation.language')),
      card(
        segRow('dictation.language', t('dictation.language.title'), t('dictation.language.desc'), ['auto', 'vi', 'en'], when),
        model('vi'),
        model('en'),
        toggleRow('dictation.on_device', t('dictation.on_device.title'), t('dictation.on_device.desc'), when)),
      sectionTitle(t('sec.dictation.listening')),
      card(
        segRow('dictation.silence_seconds', t('dictation.silence_seconds.title'), t('dictation.silence_seconds.desc'), [0, 2, 3, 5], when),
        toggleRow('dictation.sound', t('dictation.sound.title'), t('dictation.sound.desc'), when)),
      sectionTitle(t('sec.dictation.access')),
      card(
        access('speech', speech, 'dictation_allow', 'dictation_open_speech'),
        access('microphone', microphone, 'dictation_allow', 'dictation_open_microphone'),
        access('accessibility', () => (S.dictation.accessibility ? 'yes' : 'no'), null, 'dictation_open_accessibility')),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('dictation');
    const page = document.querySelector('#scroller');
    const switchOf = (path) => page.querySelector('.switch[data-path="' + path + '"]');
    sent.length = 0;
    const was = S.dictation.enabled;
    switchOf('dictation.enabled').click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'dictation.enabled' && m.value === !was), 'dictation: the switch sends its setting: ' + JSON.stringify(sent));
    switchOf('dictation.enabled').click();

    const seg = (text) => [...page.querySelectorAll('.seg button')].find((b) => b.textContent === text);
    sent.length = 0;
    seg(t('dictation.language.vi')).click();
    seg(t('dictation.silence_seconds.5')).click();
    expect(sent.some((m) => m.path === 'dictation.language' && m.value === 'vi') && sent.some((m) => m.path === 'dictation.silence_seconds' && m.value === 5), 'dictation: the language and the silence are chosen: ' + JSON.stringify(sent));
    expect(page.textContent.includes(t('dictation.model.local')), 'dictation: a model recognized on this Mac says so');
    S.dictation.models.en = { available: true, on_device: false }; runSyncs();
    expect(page.textContent.includes(t('dictation.model.server')), 'dictation: a model that needs Apple\'s servers says so');
    S.dictation.models.en = { available: true, on_device: true }; runSyncs();

    const button = (text) => [...page.querySelectorAll('.btn')].filter((b) => b.textContent === text && b.style.display !== 'none');
    sent.length = 0;
    button(t('dictation.allow'))[0].click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'dictation_allow'), 'dictation: access not asked yet is asked for');
    S.dictation.speech = 'denied'; runSyncs();
    sent.length = 0;
    button(t('dictation.open'))[0].click();
    expect(sent.some((m) => m.name === 'dictation_open_speech'), 'dictation: refused access opens System Settings: ' + JSON.stringify(sent));
    S.dictation.speech = 'ask'; runSyncs();
    expect(!page.querySelector('.dictation-status') || page.textContent.includes(t('dictation.access.yes')), 'dictation: allowed access says so');
  },
});
