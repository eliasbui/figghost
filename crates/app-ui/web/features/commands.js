// Commands: a tab of the notch that runs the commands you saved and shows what they print.
(window.__plugins = window.__plugins || []).push({
  id: 'commands',
  icon: 'terminal',
  en: {
    'nav.commands': 'Commands', 'page.commands': 'Commands',
    'page.commands.sub': 'Save a few commands, then run them from a tab of the notch and watch what they print.',
    'commands.enable.title': 'Show the Commands tab', 'commands.enable.desc': 'A tab of the notch with the commands below. Each runs when pressed, with your shell (zsh) from your home folder, and what it prints scrolls in the tab. It is a runner and not a terminal: a command gets no keyboard and no screen to draw on, one runs at a time, and one that runs for over ten minutes is stopped. Nothing it prints is kept.',
    'sec.commands.items': 'Saved commands',
    'commands.items.desc': 'A name, and the line to run, such as cd ~/project && npm test. The line runs exactly as written, as you: only put in what you would type in Terminal.',
    'commands.name': 'Name', 'commands.line': 'Line to run', 'commands.add': 'Add a command', 'commands.remove': 'Remove {name}',
    'commands.bad': 'Every command needs a name and a line, each of plain text on one line. Nothing is saved until they do.',
    'commands.full': 'There is room for {n} commands.',
  },
  vi: {
    'nav.commands': 'Lệnh', 'page.commands': 'Lệnh',
    'page.commands.sub': 'Lưu vài lệnh, rồi chạy chúng từ một tab của notch và xem chúng in ra gì.',
    'commands.enable.title': 'Hiện tab Lệnh', 'commands.enable.desc': 'Một tab của notch với các lệnh bên dưới. Mỗi lệnh chạy khi bấm, bằng shell của bạn (zsh) từ thư mục home, và những gì nó in ra cuộn trong tab. Đây là trình chạy chứ không phải terminal: lệnh không có bàn phím lẫn màn hình để vẽ, mỗi lần chỉ chạy một lệnh, và lệnh chạy quá mười phút sẽ bị dừng. Không giữ lại gì nó in ra.',
    'sec.commands.items': 'Lệnh đã lưu',
    'commands.items.desc': 'Một cái tên và dòng cần chạy, như cd ~/project && npm test. Dòng được chạy đúng như viết, với tư cách bạn: chỉ nhập những gì bạn sẽ gõ trong Terminal.',
    'commands.name': 'Tên', 'commands.line': 'Dòng lệnh', 'commands.add': 'Thêm lệnh', 'commands.remove': 'Bỏ {name}',
    'commands.bad': 'Mỗi lệnh cần một cái tên và một dòng, đều là chữ thường trên một dòng. Chưa có gì được lưu cho đến khi đủ.',
    'commands.full': 'Chỉ có chỗ cho {n} lệnh.',
  },
  build() {
    const on = () => S.commands.enabled;
    const when = { when: on };
    // The list as typed: it goes to the app only when every command in it is good, so that nothing is lost half way.
    let draft = S.commands.items.map((i) => ({ ...i }));
    let editing = false;
    const box = h('div', { class: 'commands-list' });
    const note = h('div', { class: 'row-desc commands-note', role: 'status' });
    const good = (i) => i.name.trim() && i.command.trim() && !/[\u0000-\u001f]/.test(i.name + i.command);
    const save = () => {
      const ok = draft.every(good);
      note.textContent = ok ? '' : t('commands.bad');
      note.style.display = ok ? 'none' : '';
      if (ok) send({ t: 'set', path: 'commands.items', value: draft.map((i) => ({ name: i.name.trim(), command: i.command.trim() })) });
    };
    const draw = () => {
      box.replaceChildren(...draft.map((item, k) => {
        const name = h('input', { type: 'text', class: 'text-field commands-name', placeholder: t('commands.name'), 'aria-label': t('commands.name'), maxlength: '40', spellcheck: 'false' });
        const line = h('input', { type: 'text', class: 'text-field commands-line', placeholder: t('commands.line'), 'aria-label': t('commands.line'), maxlength: '500', spellcheck: 'false' });
        name.value = item.name; line.value = item.command;
        name.addEventListener('input', () => { item.name = name.value; editing = true; });
        line.addEventListener('input', () => { item.command = line.value; editing = true; });
        for (const el of [name, line]) el.addEventListener('change', () => { editing = false; save(); });
        const gone = button('', () => { draft.splice(k, 1); draw(); save(); }, { icon: 'xmark', small: true, aria: t('commands.remove', { name: item.name || String(k + 1) }) });
        return h('div', { class: 'commands-item' }, name, line, gone);
      }));
    };
    draw();
    const add = button(t('commands.add'), () => { if (draft.length < S.commands.max) { draft.push({ name: '', command: '' }); draw(); } }, { small: true });
    reg(() => {
      add.disabled = !on() || draft.length >= S.commands.max;
      // What the app has is the truth, unless the person is typing.
      if (!editing && JSON.stringify(draft.filter(good)) !== JSON.stringify(S.commands.items) && draft.every(good)) { draft = S.commands.items.map((i) => ({ ...i })); draw(); }
    });
    return [
      card(toggleRow('commands.enabled', t('commands.enable.title'), t('commands.enable.desc'))),
      sectionTitle(t('sec.commands.items')),
      card(row(t('commands.items.desc'), null, null, when), h('div', { class: 'row block' }, box, note), row(add, null, null, when)),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('commands');
    sent.length = 0;
    const first = document.querySelector('.switch');
    const was = S.commands.enabled;
    first.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'commands.enabled' && m.value === !was), 'commands: the switch sends its setting: ' + JSON.stringify(sent));
    first.click();
    const lines = document.querySelectorAll('.commands-item');
    expect(lines.length === S.commands.items.length, 'commands: a row for each saved command');
    sent.length = 0;
    const line = lines[0].querySelector('.commands-line');
    line.value = 'echo changed';
    line.dispatchEvent(new Event('input'));
    line.dispatchEvent(new Event('change'));
    const sentItems = sent.find((m) => m.path === 'commands.items');
    expect(sentItems && sentItems.value[0].command === 'echo changed' && sentItems.value.length === lines.length, 'commands: the whole list is sent: ' + JSON.stringify(sent));
    sent.length = 0;
    [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === t('commands.add')).click();
    expect(document.querySelectorAll('.commands-item').length === lines.length + 1 && !sent.length, 'commands: a new row is empty and sends nothing');
    const blank = document.querySelectorAll('.commands-item')[lines.length];
    blank.querySelector('.commands-name').value = 'x';
    blank.querySelector('.commands-name').dispatchEvent(new Event('input'));
    blank.querySelector('.commands-name').dispatchEvent(new Event('change'));
    expect(!sent.length && document.body.textContent.includes(t('commands.bad')), 'commands: a half-written command is not sent and the page says why');
    blank.querySelector('.commands-line').value = 'ls';
    blank.querySelector('.commands-line').dispatchEvent(new Event('input'));
    blank.querySelector('.commands-line').dispatchEvent(new Event('change'));
    expect(sent.some((m) => m.path === 'commands.items' && m.value.length === lines.length + 1), 'commands: once it is whole it is sent: ' + JSON.stringify(sent));
  },
});
