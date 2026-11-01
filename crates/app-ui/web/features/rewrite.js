// Rewrite: select text anywhere, press the shortcut, and a glass window shows it fixed, more formal, shorter or in English.
(window.__plugins = window.__plugins || []).push({
  id: 'rewrite',
  icon: 'pencil.line',
  en: {
    'nav.rewrite': 'Rewrite', 'page.rewrite': 'Rewrite',
    'page.rewrite.sub': 'Select text anywhere and press the shortcut: a glass window next to the pointer shows it fixed, more formal, shorter or in English.',
    'target.rewrite': 'the Rewrite shortcut',
    'rewrite.enable.title': 'Use Rewrite', 'rewrite.enable.desc': 'The selected text is read only when you press the shortcut or choose Rewrite from the menu bar item. It is sent, over https, to the model that Jarvis is set to ask (its own service, or the model on this Mac): set the provider and its key on the Jarvis page. Nothing else is sent and nothing is kept.',
    'rewrite.model.title': 'The model', 'rewrite.model.ready': 'Jarvis is ready to be asked.',
    'rewrite.shortcut.title': 'Shortcut', 'rewrite.shortcut.desc': 'Works in any app. Return uses the result and Escape closes the window.',
    'rewrite.access.title': 'Reading the selection', 'rewrite.access.desc': 'Needs Accessibility access. An app that does not share its selection is read by copying it for a moment: your clipboard is put back as it was and the history does not record it.',
    'rewrite.access.open': 'Open settings',
    'rewrite.try.title': 'Try it', 'rewrite.try.desc': 'Rewrites a sample sentence with its mistakes, without selecting anything.', 'rewrite.try.button': 'Try it',
    'sec.rewrite.output': 'What it does',
    'rewrite.mode.title': 'Rewrite as', 'rewrite.mode.desc': 'What the shortcut asks of the model. Up to 4000 characters at a time.',
    'rewrite.mode.fix': 'Fix mistakes', 'rewrite.mode.formal': 'More formal', 'rewrite.mode.short': 'Shorter', 'rewrite.mode.english': 'Into English',
    'rewrite.replace.title': 'Replace the selection', 'rewrite.replace.desc': 'Return pastes the result over what you selected, with Command+V, and puts your clipboard back. Needs Accessibility; without it the result is copied. Off, Return only copies it.',
    'rewrite.style.title': 'Window style', 'rewrite.style.desc': 'The glass the window is made of.',
  },
  vi: {
    'nav.rewrite': 'Viết lại', 'page.rewrite': 'Viết lại',
    'page.rewrite.sub': 'Chọn văn bản ở bất kỳ đâu rồi nhấn phím tắt: một cửa sổ kính cạnh con trỏ cho thấy nó đã sửa lỗi, trang trọng hơn, ngắn hơn hoặc sang tiếng Anh.',
    'target.rewrite': 'phím tắt Viết lại',
    'rewrite.enable.title': 'Dùng Viết lại', 'rewrite.enable.desc': 'Chỉ đọc văn bản đang chọn khi bạn nhấn phím tắt hoặc chọn Viết lại trong biểu tượng trên thanh menu. Văn bản được gửi, qua https, tới mô hình mà Jarvis đang dùng (dịch vụ của nó, hoặc mô hình chạy trên máy này): hãy đặt nhà cung cấp và khóa ở trang Jarvis. Không gửi gì khác và không lưu gì.',
    'rewrite.model.title': 'Mô hình', 'rewrite.model.ready': 'Jarvis sẵn sàng để hỏi.',
    'rewrite.shortcut.title': 'Phím tắt', 'rewrite.shortcut.desc': 'Dùng ở mọi ứng dụng. Enter để dùng kết quả, Escape đóng cửa sổ.',
    'rewrite.access.title': 'Đọc phần đang chọn', 'rewrite.access.desc': 'Cần quyền Trợ năng. Với ứng dụng không chia sẻ phần đang chọn, văn bản được sao chép trong giây lát: clipboard được trả lại như cũ và lịch sử không ghi lại.',
    'rewrite.access.open': 'Mở cài đặt',
    'rewrite.try.title': 'Thử ngay', 'rewrite.try.desc': 'Viết lại một câu mẫu có lỗi, không cần chọn gì.', 'rewrite.try.button': 'Thử ngay',
    'sec.rewrite.output': 'Làm gì',
    'rewrite.mode.title': 'Viết lại theo kiểu', 'rewrite.mode.desc': 'Điều phím tắt nhờ mô hình làm. Tối đa 4000 ký tự mỗi lần.',
    'rewrite.mode.fix': 'Sửa lỗi', 'rewrite.mode.formal': 'Trang trọng hơn', 'rewrite.mode.short': 'Ngắn gọn hơn', 'rewrite.mode.english': 'Sang tiếng Anh',
    'rewrite.replace.title': 'Thay phần đang chọn', 'rewrite.replace.desc': 'Enter dán kết quả đè lên phần bạn đã chọn bằng Command+V rồi trả clipboard lại như cũ. Cần quyền Trợ năng; không có thì kết quả được sao chép. Tắt thì Enter chỉ sao chép.',
    'rewrite.style.title': 'Kiểu cửa sổ', 'rewrite.style.desc': 'Chất liệu kính của cửa sổ.',
  },
  build() {
    const on = () => S.rewrite.enabled;
    const accessPill = pill(() => (S.rewrite.accessibility ? t('chip.access.ok') : t('chip.access.no')), () => S.rewrite.accessibility);
    const model = row(t('rewrite.model.title'), '', null, { when: on });
    const note = model.querySelector('.row-desc') || model.querySelector('.row-text').appendChild(h('div', { class: 'row-desc' }));
    reg(() => { note.textContent = S.rewrite.model.ready ? t('rewrite.model.ready') : S.rewrite.model.note; });
    return [
      card(toggleRow('rewrite.enabled', t('rewrite.enable.title'), t('rewrite.enable.desc')), model),
      card(
        row(t('rewrite.shortcut.title'), t('rewrite.shortcut.desc'), hotkeyButton('rewrite', { label: t('rewrite.shortcut.title') }), { when: on }),
        row(t('rewrite.access.title'), t('rewrite.access.desc'), h('div', { class: 'row-control' }, accessPill, button(t('rewrite.access.open'), () => send({ t: 'action', name: 'open_accessibility' }), { small: true }))),
        row(t('rewrite.try.title'), t('rewrite.try.desc'), button(t('rewrite.try.button'), () => send({ t: 'action', name: 'rewrite_try' }), { small: true }), { when: on })),
      sectionTitle(t('sec.rewrite.output')),
      card(
        selectRow({ path: 'rewrite.mode', title: t('rewrite.mode.title'), desc: t('rewrite.mode.desc'), when: on,
          options: () => ['fix', 'formal', 'short', 'english'].map((v) => ({ value: v, label: t('rewrite.mode.' + v) })) }),
        toggleRow('rewrite.replace', t('rewrite.replace.title'), t('rewrite.replace.desc'), { when: on }),
        selectRow({ path: 'rewrite.material', title: t('rewrite.style.title'), desc: t('rewrite.style.desc'), when: on,
          options: () => ['glass', 'clear', 'ink', 'accent'].map((v) => ({ value: v, label: t('notch.material.' + v) })) })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('rewrite');
    sent.length = 0;
    const first = document.querySelector('.switch');
    const was = S.rewrite.enabled;
    first.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'rewrite.enabled' && m.value === !was), 'the Rewrite switch sends its setting: ' + JSON.stringify(sent));
    first.click();
    expect(document.querySelectorAll('.hotkey').length === 1, 'rewrite: one shortcut recorder');
    sent.length = 0;
    [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === t('rewrite.try.button')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'rewrite_try'), 'rewrite: Try it sends its action');
    sent.length = 0;
    const pick = document.querySelector('select.pick');
    pick.value = 'formal';
    pick.dispatchEvent(new Event('change'));
    expect(sent.some((m) => m.t === 'set' && m.path === 'rewrite.mode' && m.value === 'formal'), 'rewrite: the mode is a word: ' + JSON.stringify(sent));
    expect([...pick.querySelectorAll('option')].map((o) => o.value).join() === 'fix,formal,short,english', 'rewrite: the four modes');
    S.rewrite.model = { ready: false, note: 'No key for Anthropic.' }; runSyncs();
    expect(document.body.textContent.includes('No key for Anthropic.'), 'rewrite: what the model lacks is said');
    S.rewrite.model = { ready: true, note: '' }; runSyncs();
  },
});
