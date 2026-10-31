// Jarvis (beta): an assistant that answers questions in a chat on Liquid Glass. A shortcut opens the panel.
(window.__plugins = window.__plugins || []).push({
  id: 'jarvis',
  icon: 'sparkles',
  en: {
    'nav.jarvis': 'Jarvis', 'page.jarvis': 'Jarvis',
    'page.jarvis.sub': 'Press the shortcut, type a question and read the answer in a panel of Liquid Glass. Beta.',
    'jarvis.enable.title': 'Ask Jarvis (Beta)', 'jarvis.enable.desc': 'A chat opens in the middle of the screen. Return sends your question, Escape stops the answer and then closes the panel. Turning it on sends what you type there to the service you choose below.',
    'jarvis.shortcut.title': 'Shortcut', 'jarvis.shortcut.desc': 'Press it again to close the panel.', 'jarvis.start': 'Open Jarvis',
    'sec.jarvis.service': 'Service',
    'jarvis.provider.title': 'Service', 'jarvis.provider.desc': 'Who answers. Each needs its own key, except a model on this Mac.',
    'jarvis.provider.anthropic': 'Anthropic (Claude)', 'jarvis.provider.codex': 'Codex (ChatGPT plan)', 'jarvis.provider.openai': 'OpenAI', 'jarvis.provider.gemini': 'Google Gemini', 'jarvis.provider.deepseek': 'DeepSeek', 'jarvis.provider.local': 'On this Mac (Ollama)',
    'jarvis.model.title': 'Model', 'jarvis.model.desc': 'Leave empty for the service\'s usual one. Names change, so type the one you want.',
    'jarvis.key.title': 'Key', 'jarvis.key.desc': 'Kept in the Keychain of this Mac, never in the settings file.',
    'jarvis.key.ph': 'Paste the key', 'jarvis.key.save': 'Save', 'jarvis.key.remove': 'Remove',
    'jarvis.signin.ready': 'Signed in to Codex', 'jarvis.signin.missing': 'Codex is not signed in on this Mac: run codex login', 'jarvis.signin.expired': 'The Codex sign-in ran out: use Codex once to renew it', 'jarvis.signin.unreadable': 'The Codex sign-in file cannot be read',
    'jarvis.signin.note': 'Uses the sign-in of the Codex command line and counts against your ChatGPT plan. It is only read, never renewed or changed here. This is not an official way for other programs to use that service, and OpenAI may stop it at any time.',
    'jarvis.key.saved': 'A key is saved', 'jarvis.key.none': 'No key yet', 'jarvis.key.notneeded': 'No key needed',
    'sec.jarvis.voice': 'Voice',
    'jarvis.voice.title': 'Talk to Jarvis', 'jarvis.voice.desc': 'The panel asks what you want, listens, and sends what you said after a silence. Turn it off to only type.',
    'jarvis.language.title': 'Language spoken', 'jarvis.language.desc': 'Each language has its own model, the one macOS keeps.',
    'jarvis.language.auto': 'As the app', 'jarvis.language.vi': 'Vietnamese', 'jarvis.language.en': 'English',
    'jarvis.on_device.title': 'Only on this Mac', 'jarvis.on_device.desc': 'The audio never leaves the Mac. Off, a language this Mac cannot recognize by itself is sent to Apple to be recognized.',
    'jarvis.speak.title': 'Read the answer aloud', 'jarvis.speak.desc': 'Each sentence is read as soon as it is written, in the language it is in, with the voice of this Mac.',
    'jarvis.sound.title': 'Sounds', 'jarvis.sound.desc': 'A soft sound when Jarvis wakes, answers, and when something goes wrong.',
    'sec.jarvis.access': 'Access',
    'jarvis.speech.title': 'Speech Recognition', 'jarvis.speech.desc': 'To turn your voice into words.',
    'jarvis.microphone.title': 'Microphone', 'jarvis.microphone.desc': 'Only while Jarvis listens.',
    'jarvis.access.yes': 'Allowed', 'jarvis.access.no': 'Not allowed', 'jarvis.access.ask': 'Not asked yet',
    'jarvis.allow': 'Allow...', 'jarvis.open': 'Open System Settings',
  },
  vi: {
    'nav.jarvis': 'Jarvis', 'page.jarvis': 'Jarvis',
    'page.jarvis.sub': 'Nhấn phím tắt, gõ câu hỏi và đọc câu trả lời trong một bảng Liquid Glass. Bản thử nghiệm.',
    'jarvis.enable.title': 'Hỏi Jarvis (Beta)', 'jarvis.enable.desc': 'Một khung trò chuyện hiện ở giữa màn hình. Return để gửi câu hỏi, Esc để dừng câu trả lời rồi đóng bảng. Bật tính năng nghĩa là chữ bạn gõ ở đó được gửi tới dịch vụ bạn chọn dưới đây.',
    'jarvis.shortcut.title': 'Phím tắt', 'jarvis.shortcut.desc': 'Nhấn lần nữa để đóng bảng.', 'jarvis.start': 'Mở Jarvis',
    'sec.jarvis.service': 'Dịch vụ',
    'jarvis.provider.title': 'Dịch vụ', 'jarvis.provider.desc': 'Ai trả lời. Mỗi nơi cần khóa riêng, trừ mô hình chạy ngay trên máy này.',
    'jarvis.provider.anthropic': 'Anthropic (Claude)', 'jarvis.provider.codex': 'Codex (gói ChatGPT)', 'jarvis.provider.openai': 'OpenAI', 'jarvis.provider.gemini': 'Google Gemini', 'jarvis.provider.deepseek': 'DeepSeek', 'jarvis.provider.local': 'Trên máy này (Ollama)',
    'jarvis.model.title': 'Mô hình', 'jarvis.model.desc': 'Để trống thì dùng mô hình thường dùng của dịch vụ. Tên hay đổi nên bạn gõ tên muốn dùng.',
    'jarvis.key.title': 'Khóa', 'jarvis.key.desc': 'Được giữ trong Keychain của máy này, không bao giờ nằm trong tệp cài đặt.',
    'jarvis.key.ph': 'Dán khóa vào đây', 'jarvis.key.save': 'Lưu', 'jarvis.key.remove': 'Xóa',
    'jarvis.signin.ready': 'Đã đăng nhập Codex', 'jarvis.signin.missing': 'Codex chưa đăng nhập trên máy này: chạy codex login', 'jarvis.signin.expired': 'Phiên đăng nhập Codex đã hết hạn: dùng Codex một lần để làm mới', 'jarvis.signin.unreadable': 'Không đọc được tệp đăng nhập của Codex',
    'jarvis.signin.note': 'Dùng đăng nhập của Codex trên dòng lệnh và tính vào gói ChatGPT của bạn. Ở đây chỉ đọc, không bao giờ làm mới hay sửa. Đây không phải cách chính thức để chương trình khác dùng dịch vụ đó, và OpenAI có thể ngừng bất cứ lúc nào.',
    'jarvis.key.saved': 'Đã có khóa', 'jarvis.key.none': 'Chưa có khóa', 'jarvis.key.notneeded': 'Không cần khóa',
    'sec.jarvis.voice': 'Giọng nói',
    'jarvis.voice.title': 'Nói chuyện với Jarvis', 'jarvis.voice.desc': 'Bảng hỏi bạn muốn gì, lắng nghe, rồi gửi điều bạn nói sau một lúc im lặng. Tắt để chỉ gõ.',
    'jarvis.language.title': 'Ngôn ngữ nói', 'jarvis.language.desc': 'Mỗi ngôn ngữ có một mô hình riêng, là mô hình macOS có sẵn.',
    'jarvis.language.auto': 'Theo ứng dụng', 'jarvis.language.vi': 'Tiếng Việt', 'jarvis.language.en': 'Tiếng Anh',
    'jarvis.on_device.title': 'Chỉ trên máy này', 'jarvis.on_device.desc': 'Âm thanh không bao giờ rời khỏi máy. Nếu tắt, ngôn ngữ nào máy chưa tự nhận dạng được sẽ được gửi tới Apple để nhận dạng.',
    'jarvis.speak.title': 'Đọc to câu trả lời', 'jarvis.speak.desc': 'Từng câu được đọc ngay khi viết xong, bằng ngôn ngữ của câu đó, với giọng của máy này.',
    'jarvis.sound.title': 'Âm thanh', 'jarvis.sound.desc': 'Một tiếng nhỏ khi Jarvis thức dậy, trả lời xong và khi có lỗi.',
    'sec.jarvis.access': 'Quyền truy cập',
    'jarvis.speech.title': 'Nhận dạng giọng nói', 'jarvis.speech.desc': 'Để chuyển giọng nói của bạn thành chữ.',
    'jarvis.microphone.title': 'Micrô', 'jarvis.microphone.desc': 'Chỉ trong lúc Jarvis lắng nghe.',
    'jarvis.access.yes': 'Đã cho phép', 'jarvis.access.no': 'Chưa cho phép', 'jarvis.access.ask': 'Chưa hỏi',
    'jarvis.allow': 'Cho phép...', 'jarvis.open': 'Mở Cài đặt hệ thống',
  },
  build() {
    const on = () => S.jarvis.enabled;
    const when = { when: on };

    const model = h('input', { type: 'text', class: 'text-field jarvis-model', 'aria-label': t('jarvis.model.title'), maxlength: '80', spellcheck: 'false', autocomplete: 'off', autocapitalize: 'off' });
    model.addEventListener('change', () => change('jarvis.model', model.value.trim()));
    reg(() => {
      model.placeholder = S.jarvis.default_model;
      if (document.activeElement !== model) model.value = S.jarvis.model;
    });

    const field = h('input', { type: 'password', class: 'text-field', 'aria-label': t('jarvis.key.title'), placeholder: t('jarvis.key.ph'), maxlength: '400', spellcheck: 'false', autocomplete: 'off', autocapitalize: 'off' });
    const status = h('span', { class: 'row-desc jarvis-key-status' });
    const save = button(t('jarvis.key.save'), () => {
      const value = field.value.trim();
      if (!value) return;
      send({ t: 'set', path: 'jarvis.key', value });
      field.value = '';
    }, { small: true });
    const remove = button(t('jarvis.key.remove'), () => send({ t: 'set', path: 'jarvis.key', value: '' }), { small: true });
    field.addEventListener('keydown', (e) => { if (e.key === 'Enter') save.click(); });
    reg(() => {
      const needed = S.jarvis.needs_key;
      status.textContent = S.jarvis.signin ? t('jarvis.signin.' + S.jarvis.signin) : t(!needed ? 'jarvis.key.notneeded' : S.jarvis.key ? 'jarvis.key.saved' : 'jarvis.key.none');
      field.style.display = needed ? '' : 'none';
      save.style.display = needed ? '' : 'none';
      remove.style.display = needed && S.jarvis.key ? '' : 'none';
    });

    // One row of access: what it is now, and the one button that moves it on.
    const act = (name) => send({ t: 'action', name });
    const access = (key, state, ask, open) => {
      const status = h('span', { class: 'row-desc jarvis-key-status' });
      const allow = button(t('jarvis.allow'), () => act(ask), { small: true });
      const settings = button(t('jarvis.open'), () => act(open), { small: true });
      reg(() => {
        const now = state();
        status.textContent = t('jarvis.access.' + now);
        allow.style.display = now === 'ask' ? '' : 'none';
        settings.style.display = now === 'no' ? '' : 'none';
      });
      return row(t('jarvis.' + key + '.title'), t('jarvis.' + key + '.desc'), h('div', { class: 'jarvis-key' }, status, allow, settings), when);
    };
    const speech = () => ({ allowed: 'yes', denied: 'no' })[S.jarvis.speech] || 'ask';
    const microphone = () => (S.jarvis.microphone ? 'yes' : S.jarvis.speech === 'ask' ? 'ask' : 'no');

    // Small print under the service: what the Codex route is, and that it is not an official one.
    const note = h('p', { class: 'jarvis-note', text: t('jarvis.signin.note') });
    reg(() => { note.style.display = on() && S.jarvis.provider === 'codex' ? '' : 'none'; });

    return [
      card(toggleRow('jarvis.enabled', t('jarvis.enable.title'), t('jarvis.enable.desc'))),
      card(row(t('jarvis.shortcut.title'), t('jarvis.shortcut.desc'), hotkeyButton('jarvis', { allowClear: true, label: t('jarvis.start') }), when)),
      sectionTitle(t('sec.jarvis.service')),
      card(
        selectRow({
          path: 'jarvis.provider', title: t('jarvis.provider.title'), desc: t('jarvis.provider.desc'),
          options: () => ['anthropic', 'codex', 'openai', 'gemini', 'deepseek', 'local'].map((v) => ({ value: v, label: t('jarvis.provider.' + v) })),
          when: on,
        }),
        row(t('jarvis.model.title'), t('jarvis.model.desc'), model, when),
        row(t('jarvis.key.title'), t('jarvis.key.desc'), h('div', { class: 'jarvis-key' }, status, field, save, remove), when)),
      note,
      sectionTitle(t('sec.jarvis.voice')),
      card(
        toggleRow('jarvis.voice', t('jarvis.voice.title'), t('jarvis.voice.desc'), when),
        segRow('jarvis.language', t('jarvis.language.title'), t('jarvis.language.desc'), ['auto', 'vi', 'en'], when),
        toggleRow('jarvis.on_device', t('jarvis.on_device.title'), t('jarvis.on_device.desc'), when),
        toggleRow('jarvis.speak', t('jarvis.speak.title'), t('jarvis.speak.desc'), when),
        toggleRow('jarvis.sound', t('jarvis.sound.title'), t('jarvis.sound.desc'), when)),
      sectionTitle(t('sec.jarvis.access')),
      card(
        access('speech', speech, 'jarvis_allow', 'jarvis_open_speech'),
        access('microphone', microphone, 'jarvis_allow', 'jarvis_open_microphone')),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('jarvis');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const was = S.jarvis.enabled;
    page.querySelector('.switch[data-path="jarvis.enabled"]').click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'jarvis.enabled' && m.value === !was), 'jarvis: the switch sends its setting: ' + JSON.stringify(sent));
    page.querySelector('.switch[data-path="jarvis.enabled"]').click();

    const model = page.querySelector('.jarvis-model');
    expect(model.placeholder === S.jarvis.default_model, 'jarvis: an empty model shows the service\'s own as its placeholder');
    sent.length = 0;
    model.value = ' gpt-4o '; model.dispatchEvent(new Event('change'));
    expect(sent.some((m) => m.path === 'jarvis.model' && m.value === 'gpt-4o'), 'jarvis: the model is sent trimmed: ' + JSON.stringify(sent));

    const field = page.querySelector('input[type="password"]');
    const buttons = (text) => [...page.querySelectorAll('.btn')].filter((b) => b.textContent === text);
    sent.length = 0;
    field.value = 'sk-test'; buttons(t('jarvis.key.save'))[0].click();
    expect(sent.some((m) => m.path === 'jarvis.key' && m.value === 'sk-test'), 'jarvis: the key is sent once: ' + JSON.stringify(sent));
    expect(field.value === '', 'jarvis: the key does not stay in the page');
    S.jarvis.key = true; runSyncs();
    expect(page.textContent.includes(t('jarvis.key.saved')) && buttons(t('jarvis.key.remove'))[0].style.display !== 'none', 'jarvis: a saved key says so and can be removed');
    S.jarvis.provider = 'local'; S.jarvis.needs_key = false; runSyncs();
    expect(page.textContent.includes(t('jarvis.key.notneeded')) && field.style.display === 'none', 'jarvis: a model on this Mac needs no key');
    S.jarvis.provider = 'codex'; S.jarvis.needs_key = false; S.jarvis.signin = 'expired'; runSyncs();
    expect(page.textContent.includes(t('jarvis.signin.expired')) && field.style.display === 'none', 'jarvis: an expired Codex sign-in says what to do, and no key is asked for');
    expect(page.querySelector('.jarvis-note').style.display !== 'none', 'jarvis: the Codex route says it is not an official one');
    S.jarvis.signin = 'ready'; runSyncs();
    expect(page.textContent.includes(t('jarvis.signin.ready')), 'jarvis: a Codex sign-in that is there says so');
    S.jarvis.provider = 'anthropic'; S.jarvis.needs_key = true; S.jarvis.key = false; S.jarvis.signin = ''; runSyncs();
    expect(page.querySelector('.jarvis-note').style.display === 'none', 'jarvis: the note is only for Codex');

    const seg = (text) => [...page.querySelectorAll('.seg button')].find((b) => b.textContent === text);
    sent.length = 0;
    seg(t('jarvis.language.vi')).click();
    expect(sent.some((m) => m.path === 'jarvis.language' && m.value === 'vi'), 'jarvis: the language is chosen: ' + JSON.stringify(sent));
    for (const path of ['jarvis.voice', 'jarvis.speak', 'jarvis.sound', 'jarvis.on_device']) {
      sent.length = 0;
      page.querySelector('.switch[data-path="' + path + '"]').click();
      expect(sent.some((m) => m.t === 'set' && m.path === path), 'jarvis: ' + path + ' is a switch: ' + JSON.stringify(sent));
      page.querySelector('.switch[data-path="' + path + '"]').click();
    }
    const visible = (text) => [...page.querySelectorAll('.btn')].filter((b) => b.textContent === text && b.style.display !== 'none');
    sent.length = 0;
    visible(t('jarvis.allow'))[0].click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'jarvis_allow'), 'jarvis: access not asked yet is asked for');
    S.jarvis.speech = 'denied'; runSyncs();
    sent.length = 0;
    visible(t('jarvis.open'))[0].click();
    expect(sent.some((m) => m.name === 'jarvis_open_speech'), 'jarvis: refused access opens System Settings: ' + JSON.stringify(sent));
    S.jarvis.speech = 'ask'; runSyncs();
  },
});
