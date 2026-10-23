// Snippets: type an abbreviation in any app and it is replaced by a longer text.
(window.__plugins = window.__plugins || []).push({
  id: 'snippets',
  icon: 'text.cursor',
  en: {
    'nav.snippets': 'Snippets', 'page.snippets': 'Snippets',
    'page.snippets.sub': 'Type a short abbreviation in any app and it is replaced by the text you saved: an email reply, an address, a signature, a piece of code.',
    'snippets.enable.title': 'Replace abbreviations', 'snippets.enable.desc': 'Works with Vietnamese typing on or off. Nothing is replaced in password fields, where macOS hides the keys from every app.',
    'snippets.access.title': 'Accessibility', 'snippets.access.desc': 'Needed to read the keys and put the text in.', 'snippets.access.open': 'Open settings',
    'snippets.other.title': '{name} is running',
    'snippets.other.desc': 'Two input methods would both rewrite what you type, so Figghost leaves the keys alone while {name} runs. Quit one of the two.',
    'sec.snippets.edit': 'Add a snippet', 'sec.snippets.editing': 'Edit snippet', 'sec.snippets.list': 'Your snippets', 'sec.snippets.how': 'How it works',
    'snippets.abbr': 'Abbreviation', 'snippets.abbr.ph': ';sig',
    'snippets.abbr.hint': 'Begin it with a character like ; so ordinary words are never replaced by accident, for example ;sig or ;addr. It is replaced the moment its last key is typed.',
    'snippets.text': 'Text', 'snippets.text.ph': 'What the abbreviation turns into. Line breaks are kept.',
    'snippets.add': 'Add snippet', 'snippets.save': 'Save changes',
    'snippets.none': 'No snippets yet. Add the first one above.',
    'snippets.lines': '{n} lines', 'snippets.chars': '{n} characters',
    'snippets.shadow': '{abbr} is finished first, so this one can never be typed in full.',
    'snippets.edit': 'Edit', 'snippets.remove': 'Remove',
    'snippets.remove.title': 'Remove this snippet?', 'snippets.remove.body': 'The abbreviation {abbr} will stop being replaced.',
    'snippets.how.type.title': 'Short text is typed', 'snippets.how.type.desc': 'A text of one short line is typed in place of the abbreviation, key by key, as fast as the app takes it.',
    'snippets.how.paste.title': 'Long text and line breaks are pasted', 'snippets.how.paste.desc': 'Typing a line break would press Return, which sends a message in a chat app. So these are pasted: your clipboard is kept, the text goes on it for a moment, and your clipboard is put back right after. Figghost does not add the text to the clipboard history.',
    'snippets.how.telex.title': 'Works with Telex and VNI', 'snippets.how.telex.desc': 'The abbreviation is read from the keys you press, so ;dd is found even though Telex shows it as ;đ. Keys are read by their place on a US keyboard, so an abbreviation may hold letters, digits and the punctuation of one.',
    'snippets.how.stop.title': 'When it stops looking', 'snippets.how.stop.desc': 'A click, an arrow key, Return, Tab, a space, a shortcut with Command, Control or Option, or another app coming to the front starts the abbreviation again. Backspace takes the last key back.',
    'fault.snippets_empty': 'Type an abbreviation first.', 'fault.snippets_long': 'An abbreviation may have at most 32 characters.',
    'fault.snippets_space': 'An abbreviation cannot contain spaces or line breaks.', 'fault.snippets_untypeable': 'Use only the letters, digits and punctuation that a US keyboard types.',
    'fault.snippets_no_text': 'Type the text it should turn into.', 'fault.snippets_text_long': 'A snippet may have at most 20,000 characters.',
    'fault.snippets_duplicate': 'Another snippet already uses that abbreviation.', 'fault.snippets_full': 'There can be at most 500 snippets.',
  },
  vi: {
    'nav.snippets': 'Gõ tắt', 'page.snippets': 'Gõ tắt',
    'page.snippets.sub': 'Gõ một viết tắt ngắn ở bất kỳ ứng dụng nào, nó sẽ được thay bằng đoạn văn bạn đã lưu: thư trả lời, địa chỉ, chữ ký, đoạn code.',
    'snippets.enable.title': 'Thay viết tắt', 'snippets.enable.desc': 'Chạy được cả khi gõ tiếng Việt bật hay tắt. Ô mật khẩu không bị thay, vì macOS giấu phím khỏi mọi ứng dụng ở đó.',
    'snippets.access.title': 'Trợ năng', 'snippets.access.desc': 'Cần để đọc phím và chèn đoạn văn.', 'snippets.access.open': 'Mở cài đặt',
    'snippets.other.title': '{name} đang chạy',
    'snippets.other.desc': 'Hai bộ gõ cùng chạy sẽ cùng viết lại những gì bạn gõ, nên Figghost để nguyên các phím khi {name} đang chạy. Hãy thoát một trong hai.',
    'sec.snippets.edit': 'Thêm đoạn văn', 'sec.snippets.editing': 'Sửa đoạn văn', 'sec.snippets.list': 'Các đoạn văn của bạn', 'sec.snippets.how': 'Cách hoạt động',
    'snippets.abbr': 'Viết tắt', 'snippets.abbr.ph': ';sig',
    'snippets.abbr.hint': 'Nên bắt đầu bằng một ký tự như ; để chữ thường không bị thay nhầm, ví dụ ;sig hoặc ;addr. Viết tắt được thay ngay khi gõ xong phím cuối.',
    'snippets.text': 'Đoạn văn', 'snippets.text.ph': 'Nội dung mà viết tắt sẽ biến thành. Xuống dòng được giữ nguyên.',
    'snippets.add': 'Thêm đoạn văn', 'snippets.save': 'Lưu thay đổi',
    'snippets.none': 'Chưa có đoạn văn nào. Hãy thêm cái đầu tiên ở trên.',
    'snippets.lines': '{n} dòng', 'snippets.chars': '{n} ký tự',
    'snippets.shadow': '{abbr} được thay trước khi gõ xong, nên viết tắt này không bao giờ gõ đủ được.',
    'snippets.edit': 'Sửa', 'snippets.remove': 'Xóa',
    'snippets.remove.title': 'Xóa đoạn văn này?', 'snippets.remove.body': 'Viết tắt {abbr} sẽ không còn được thay nữa.',
    'snippets.how.type.title': 'Đoạn ngắn được gõ ra', 'snippets.how.type.desc': 'Đoạn văn một dòng ngắn được gõ vào chỗ viết tắt, từng phím, nhanh hết mức ứng dụng nhận được.',
    'snippets.how.paste.title': 'Đoạn dài và có xuống dòng được dán', 'snippets.how.paste.desc': 'Gõ xuống dòng nghĩa là nhấn Return, mà Return sẽ gửi tin trong ứng dụng chat. Vì vậy những đoạn này được dán: clipboard của bạn được giữ lại, đoạn văn nằm trên clipboard trong chốc lát rồi clipboard được trả về như cũ. Figghost không đưa đoạn văn vào lịch sử clipboard.',
    'snippets.how.telex.title': 'Chạy với Telex và VNI', 'snippets.how.telex.desc': 'Viết tắt được nhận theo các phím bạn nhấn, nên ;dd vẫn được tìm thấy dù Telex hiện nó là ;đ. Phím được đọc theo vị trí trên bàn phím Mỹ, nên viết tắt chỉ gồm chữ cái, chữ số và dấu câu của bàn phím đó.',
    'snippets.how.stop.title': 'Khi nào thôi theo dõi', 'snippets.how.stop.desc': 'Một cú nhấp chuột, phím mũi tên, Return, Tab, dấu cách, phím tắt có Command, Control hoặc Option, hay ứng dụng khác lên trước đều bắt đầu lại từ đầu. Backspace lùi lại một phím.',
    'fault.snippets_empty': 'Hãy gõ viết tắt trước.', 'fault.snippets_long': 'Viết tắt dài tối đa 32 ký tự.',
    'fault.snippets_space': 'Viết tắt không được có dấu cách hay xuống dòng.', 'fault.snippets_untypeable': 'Chỉ dùng chữ cái, chữ số và dấu câu mà bàn phím Mỹ gõ được.',
    'fault.snippets_no_text': 'Hãy gõ đoạn văn mà nó sẽ biến thành.', 'fault.snippets_text_long': 'Một đoạn văn dài tối đa 20.000 ký tự.',
    'fault.snippets_duplicate': 'Đã có đoạn văn khác dùng viết tắt này.', 'fault.snippets_full': 'Tối đa 500 đoạn văn.',
  },
  build() {
    const on = () => S.snippets.enabled;
    const when = { when: on };
    const count = (text) => [...text].length;

    const otherTitle = h('span');
    const otherDesc = h('div', { class: 'row-desc' });
    const otherCard = card(h('div', { class: 'row' }, h('div', { class: 'row-text' }, h('div', { class: 'row-title' }, otherTitle), otherDesc)));
    reg(() => {
      const name = S.snippets.other;
      otherCard.hidden = !(on() && name);
      if (name) {
        otherTitle.textContent = t('snippets.other.title', { name });
        otherDesc.textContent = t('snippets.other.desc', { name });
      }
    });

    // The form adds a snippet, or changes the one being edited. The same rules as the app's, so the person sees the
    // reason at once; the app checks again.
    let editing = null;
    let pending = null;
    const abbr = h('input', { type: 'text', class: 'text-field snip-abbr', placeholder: t('snippets.abbr.ph'), 'aria-label': t('snippets.abbr'), spellcheck: 'false', autocomplete: 'off', autocapitalize: 'off', autocorrect: 'off' });
    const body = h('textarea', { class: 'snip-text', rows: '5', placeholder: t('snippets.text.ph'), 'aria-label': t('snippets.text'), spellcheck: 'false' });
    const error = h('div', { class: 'row-desc snip-error', role: 'alert' });
    const problem = () => {
      const s = S.snippets;
      const a = abbr.value;
      if (!a) return 'empty';
      if (count(a) > s.max_abbr) return 'long';
      if (/[\s\u0000-\u001f\u007f]/.test(a)) return 'space';
      if (!/^[!-~]+$/.test(a)) return 'untypeable';
      if (s.items.some((i) => i.abbr === a && i.abbr !== editing)) return 'duplicate';
      if (!editing && s.items.length >= s.max_items) return 'full';
      if (!body.value) return 'no_text';
      if (count(body.value) > s.max_text) return 'text_long';
      return '';
    };
    const submit = () => {
      if (problem()) return;
      const value = { abbr: abbr.value, text: body.value };
      pending = value;
      if (editing === null) send({ t: 'set', path: 'snippets.add', value });
      else send({ t: 'set', path: 'snippets.update', value: { ...value, from: editing } });
    };
    const add = button('', submit, { primary: true, small: true });
    const cancel = button(t('cancel'), () => clear(), { small: true });
    const heading = h('div', { class: 'section-title' });
    const clear = () => {
      editing = null;
      pending = null;
      abbr.value = '';
      body.value = '';
      runSyncs();
    };
    const paint = () => {
      const p = problem();
      const started = abbr.value || body.value;
      error.textContent = p && started && !(p === 'no_text' && !body.value && !abbr.value) ? t('fault.snippets_' + p) : '';
      add.disabled = !!p;
      add.textContent = editing === null ? t('snippets.add') : t('snippets.save');
      cancel.style.display = editing === null ? 'none' : '';
      heading.textContent = editing === null ? t('sec.snippets.edit') : t('sec.snippets.editing');
    };
    abbr.addEventListener('input', paint);
    body.addEventListener('input', paint);
    abbr.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); submit(); } });
    reg(() => {
      // The edit is done once the app's list holds what was sent.
      if (pending && S.snippets.items.some((i) => i.abbr === pending.abbr && i.text === pending.text)) clear();
      paint();
    });

    const preview = (text) => {
      const lines = text.split('\n').filter((l) => l.trim());
      const joined = lines.slice(0, 2).join(' / ');
      return joined.length > 140 ? joined.slice(0, 140) + '...' : joined;
    };
    const editItem = (item) => {
      editing = item.abbr;
      pending = null;
      abbr.value = item.abbr;
      body.value = item.text;
      runSyncs();
      abbr.focus();
      abbr.scrollIntoView({ block: 'center', behavior: 'smooth' });
    };
    const removeItem = (item) => confirmDialog(
      { title: t('snippets.remove.title'), body: t('snippets.remove.body', { abbr: item.abbr }), confirm: t('snippets.remove') },
      () => send({ t: 'set', path: 'snippets.remove', value: item.abbr }));
    const itemRow = (item) => {
      const lines = item.text.split('\n').length;
      const meta = lines > 1 ? t('snippets.lines', { n: lines }) : t('snippets.chars', { n: count(item.text) });
      const tool = (label, glyph, onclick) => button('', onclick, { small: true, icon: glyph, aria: label + ': ' + item.abbr });
      return h('div', { class: 'snip-item' },
        h('code', { class: 'snip-chip', text: item.abbr }),
        h('div', { class: 'snip-main' },
          h('div', { class: 'snip-preview', text: preview(item.text) }),
          h('div', { class: 'row-desc', text: meta }),
          item.shadowed_by && h('div', { class: 'row-desc snip-warn', text: t('snippets.shadow', { abbr: item.shadowed_by }) })),
        h('span', { class: 'snip-tools' },
          tool(t('snippets.edit'), 'pencil', () => editItem(item)),
          tool(t('snippets.remove'), 'trash', () => removeItem(item))));
    };
    const list = h('div', { class: 'snip-list' });
    let drawn = null;
    reg(() => {
      const items = S.snippets.items;
      const key = JSON.stringify(items) + lang;
      if (key === drawn) return;
      drawn = key;
      list.replaceChildren(...(items.length ? items.map(itemRow) : [h('div', { class: 'row-desc', text: t('snippets.none') })]));
    });

    const accessPill = pill(() => (S.snippets.accessibility ? t('access.granted') : t('access.missing')), () => S.snippets.accessibility);
    const form = h('div', { class: 'snip-form' },
      h('label', { class: 'snip-label' }, t('snippets.abbr'), abbr),
      h('div', { class: 'row-desc', text: t('snippets.abbr.hint') }),
      h('label', { class: 'snip-label' }, t('snippets.text'), body),
      error,
      h('div', { class: 'snip-actions' }, add, cancel));
    reg(() => { form.classList.toggle('disabled', !on()); });
    const listCard = card(row(null, null, list, { block: true, when: on }));

    return [
      card(toggleRow('snippets.enabled', t('snippets.enable.title'), t('snippets.enable.desc'))),
      otherCard,
      card(row(t('snippets.access.title'), t('snippets.access.desc'), [
        accessPill,
        button(t('snippets.access.open'), () => send({ t: 'action', name: 'open_accessibility' }), { small: true }),
      ], when)),
      heading,
      card(row(null, null, form, { block: true, when: on })),
      sectionTitle(t('sec.snippets.list')),
      listCard,
      sectionTitle(t('sec.snippets.how')),
      card(
        infoRow(t('snippets.how.type.title'), t('snippets.how.type.desc')),
        infoRow(t('snippets.how.paste.title'), t('snippets.how.paste.desc')),
        infoRow(t('snippets.how.telex.title'), t('snippets.how.telex.desc')),
        infoRow(t('snippets.how.stop.title'), t('snippets.how.stop.desc'))),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('snippets');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const master = page.querySelector('.switch');
    const was = S.snippets.enabled;
    master.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'snippets.enabled' && m.value === !was), 'snippets: the switch sends its setting: ' + JSON.stringify(sent));
    master.click();

    const rows = () => [...page.querySelectorAll('.snip-item')];
    expect(rows().length === S.snippets.items.length, 'snippets: a row for each snippet');
    expect(rows()[0].querySelector('.snip-chip').textContent === ';sig' && rows()[0].textContent.includes('Best regards'), 'snippets: a row shows the abbreviation and a preview of the text');
    expect(rows()[0].textContent.includes(t('snippets.lines', { n: 4 })), 'snippets: a text of several lines says how many');
    expect(rows().filter((r) => r.querySelector('.snip-warn')).length === 1 && rows()[3].querySelector('.snip-warn').textContent.includes(';ty'), 'snippets: an abbreviation that can never be finished says which one is in its way');

    // The form: a reason for each refusal, shown as the person types.
    const abbr = page.querySelector('input.snip-abbr');
    const text = page.querySelector('textarea.snip-text');
    const submit = page.querySelector('.snip-actions .btn.primary');
    const cancelButton = [...page.querySelectorAll('.snip-actions .btn')].find((b) => b.textContent === t('cancel'));
    expect(cancelButton.style.display === 'none', 'snippets: cancel is offered only while editing');
    const fill = (a, x) => {
      abbr.value = a; abbr.dispatchEvent(new Event('input'));
      text.value = x; text.dispatchEvent(new Event('input'));
    };
    const error = () => page.querySelector('.snip-error').textContent;
    expect(submit.disabled, 'snippets: nothing to add yet');
    for (const [a, x, code] of [
      [';sig', 'x', 'duplicate'], ['a b', 'x', 'space'], ['a\tb', 'x', 'space'], ['x'.repeat(S.snippets.max_abbr + 1), 'x', 'long'],
      ['dđ', 'x', 'untypeable'], [';new', '', 'no_text'], [';new', 'y'.repeat(S.snippets.max_text + 1), 'text_long'],
    ]) {
      fill(a, x);
      expect(submit.disabled && error() === t('fault.snippets_' + code), 'snippets: ' + code + ' is refused with its reason, got: ' + error());
    }
    sent.length = 0;
    fill(';new', 'Line one\nLine two');
    expect(!submit.disabled && error() === '', 'snippets: a good snippet can be added');
    submit.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'snippets.add' && m.value.abbr === ';new' && m.value.text === 'Line one\nLine two'), 'snippets: adding sends the abbreviation and the text: ' + JSON.stringify(sent));
    // The form stays until the app's list holds the new snippet.
    expect(abbr.value === ';new', 'snippets: the form is kept until the app has taken the snippet');
    S.snippets.items.push({ abbr: ';new', text: 'Line one\nLine two', shadowed_by: '' }); runSyncs();
    expect(abbr.value === '' && text.value === '' && rows().length === S.snippets.items.length, 'snippets: the form is cleared and the list shows the snippet');

    // Editing fills the form with the snippet and sends the old abbreviation along with the new values.
    const tools = (r) => [...r.querySelectorAll('.snip-tools .btn')];
    tools(rows()[1])[0].click();
    expect(abbr.value === ';addr' && text.value === S.snippets.items[1].text, 'snippets: editing fills the form');
    expect(page.textContent.includes(t('snippets.save')) && [...page.querySelectorAll('.snip-actions .btn')].find((b) => b.textContent === t('cancel')).style.display !== 'none', 'snippets: editing offers to save or cancel');
    fill(';addr', 'A new address');
    expect(error() === '', 'snippets: keeping the abbreviation of the snippet being edited is not a duplicate: ' + error());
    sent.length = 0;
    [...page.querySelectorAll('.snip-actions .btn')].find((b) => b.textContent === t('snippets.save')).click();
    expect(sent.some((m) => m.path === 'snippets.update' && m.value.from === ';addr' && m.value.abbr === ';addr' && m.value.text === 'A new address'), 'snippets: saving an edit names the snippet it changes: ' + JSON.stringify(sent));
    [...page.querySelectorAll('.snip-actions .btn')].find((b) => b.textContent === t('cancel')).click();
    expect(abbr.value === '' && !page.textContent.includes(t('snippets.save')), 'snippets: cancel leaves the edit');

    // Removing asks first.
    sent.length = 0;
    tools(rows()[0])[1].click();
    expect(sent.length === 0 && document.querySelector('#modal.show'), 'snippets: removing asks before it sends');
    document.querySelector('#modal .btn.primary').click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'snippets.remove' && m.value === ';sig'), 'snippets: removing names the abbreviation: ' + JSON.stringify(sent));

    // Another input method shows a notice with its name; none shows nothing.
    const notice = () => [...page.querySelectorAll('.card')].find((c) => c.textContent.includes(t('snippets.other.title', { name: 'EVKey' })));
    expect(!notice() || notice().hidden, 'snippets: no notice without another input method');
    S.snippets.other = 'EVKey'; runSyncs();
    expect(notice() && !notice().hidden, 'snippets: the notice names the other input method');
    S.snippets.other = null; runSyncs();
    S.snippets.items.pop(); runSyncs();
  },
});
