// Translate: select text anywhere, press the shortcut, and a glass window translates it.
(window.__plugins = window.__plugins || []).push({
  id: 'translate',
  icon: 'character.bubble',
  en: {
    'nav.translate': 'Translate', 'page.translate': 'Translate',
    'page.translate.sub': 'Select text anywhere and press the shortcut: a glass window next to the pointer translates it into the language you choose.',
    'target.translate': 'the Translate shortcut',
    'translate.enable.title': 'Use Translate', 'translate.enable.desc': 'The selected text is read only when you press the shortcut or choose Translate from the menu bar item. It is sent to api.mymemory.translated.net to be translated, which is the only thing sent.',
    'translate.shortcut.title': 'Shortcut', 'translate.shortcut.desc': 'Works in any app. Return or a click copies the translation and Escape closes the window.',
    'translate.access.title': 'Reading the selection', 'translate.access.desc': 'Needs Accessibility access. An app that does not share its selection is read by copying it for a moment: your clipboard is put back as it was and the history does not record it.',
    'translate.access.open': 'Open settings',
    'translate.try.title': 'Try it', 'translate.try.desc': 'Translates a sample sentence next to the pointer, without selecting anything.', 'translate.try.button': 'Try it',
    'sec.translate.output': 'Output',
    'translate.style.title': 'Window style', 'translate.style.desc': 'The glass the translation window is made of.',
    'translate.lang.title': 'Translate into', 'translate.lang.desc': 'The language that translations go into. The language of the text is detected for you. Up to 450 characters at a time.', 'translate.lang.auto': 'Same as the app',
  },
  vi: {
    'nav.translate': 'Dịch', 'page.translate': 'Dịch',
    'page.translate.sub': 'Chọn văn bản ở bất kỳ đâu rồi nhấn phím tắt: một cửa sổ kính cạnh con trỏ sẽ dịch nó sang ngôn ngữ bạn chọn.',
    'target.translate': 'phím tắt Dịch',
    'translate.enable.title': 'Dùng Dịch', 'translate.enable.desc': 'Chỉ đọc văn bản đang chọn khi bạn nhấn phím tắt hoặc chọn Dịch trong biểu tượng trên thanh menu. Văn bản được gửi tới api.mymemory.translated.net để dịch, đây là thứ duy nhất được gửi đi.',
    'translate.shortcut.title': 'Phím tắt', 'translate.shortcut.desc': 'Dùng ở mọi ứng dụng. Enter hoặc nhấp chuột để sao chép bản dịch, Escape đóng cửa sổ.',
    'translate.access.title': 'Đọc phần đang chọn', 'translate.access.desc': 'Cần quyền Trợ năng. Với ứng dụng không chia sẻ phần đang chọn, văn bản được sao chép trong giây lát: clipboard được trả lại như cũ và lịch sử không ghi lại.',
    'translate.access.open': 'Mở cài đặt',
    'translate.try.title': 'Thử ngay', 'translate.try.desc': 'Dịch một câu mẫu cạnh con trỏ, không cần chọn gì.', 'translate.try.button': 'Thử ngay',
    'sec.translate.output': 'Đầu ra',
    'translate.style.title': 'Kiểu cửa sổ', 'translate.style.desc': 'Chất liệu kính của cửa sổ bản dịch.',
    'translate.lang.title': 'Dịch sang', 'translate.lang.desc': 'Ngôn ngữ mà bản dịch hướng tới. Ngôn ngữ của văn bản được tự nhận biết. Tối đa 450 ký tự mỗi lần.', 'translate.lang.auto': 'Theo ứng dụng',
  },
  build() {
    const accessPill = pill(() => (S.translate.accessibility ? t('chip.access.ok') : t('chip.access.no')), () => S.translate.accessibility);
    const language = h('select', { class: 'pick', 'aria-label': t('translate.lang.title') });
    reg(() => {
      language.replaceChildren(h('option', { value: 'auto', text: t('translate.lang.auto') }),
        ...S.translate.languages.map((l) => h('option', { value: l.code, text: l.name })));
      language.value = S.translate.translate_to;
    });
    language.addEventListener('change', () => change('translate.translate_to', language.value));
    return [
      card(toggleRow('translate.enabled', t('translate.enable.title'), t('translate.enable.desc'))),
      card(
        row(t('translate.shortcut.title'), t('translate.shortcut.desc'), hotkeyButton('translate', { label: t('translate.shortcut.title') }), { when: () => S.translate.enabled }),
        row(t('translate.access.title'), t('translate.access.desc'), h('div', { class: 'row-control' }, accessPill, button(t('translate.access.open'), () => send({ t: 'action', name: 'open_accessibility' }), { small: true }))),
        row(t('translate.try.title'), t('translate.try.desc'), button(t('translate.try.button'), () => send({ t: 'action', name: 'translate_try' }), { small: true }), { when: () => S.translate.enabled })),
      sectionTitle(t('sec.translate.output')),
      card(row(t('translate.lang.title'), t('translate.lang.desc'), language),
        selectRow({ path: 'translate.material', title: t('translate.style.title'), desc: t('translate.style.desc'),
          options: () => ['glass', 'clear', 'ink', 'accent'].map((v) => ({ value: v, label: t('notch.material.' + v) })) })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('translate');
    sent.length = 0;
    const first = document.querySelector('.switch');
    const was = S.translate.enabled;
    first.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'translate.enabled' && m.value === !was), 'the Translate switch sends its setting: ' + JSON.stringify(sent));
    first.click();
    expect(document.querySelectorAll('.hotkey').length === 1, 'one shortcut recorder');
    sent.length = 0;
    const tryIt = [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === t('translate.try.button'));
    expect(!!tryIt, 'a Try it button');
    tryIt.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'translate_try'), 'Try it sends its action');
    sent.length = 0;
    const pick = document.querySelector('select.pick');
    pick.value = 'vi';
    pick.dispatchEvent(new Event('change'));
    expect(sent.some((m) => m.t === 'set' && m.path === 'translate.translate_to' && m.value === 'vi'), 'the language is sent: ' + JSON.stringify(sent));
    const options = [...pick.querySelectorAll('option')].map((o) => o.value);
    expect(options[0] === 'auto' && options.includes('vi') && options.includes('en'), 'the language list: ' + options.join());
  },
});
