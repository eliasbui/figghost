// The command palette: a shortcut opens a small window; what is typed picks a tab, a page, a feature to turn on or off, the
// agent that waits, a speed test or a reminder. The window is this same page told which canvas to show (see page.rs).
(window.__plugins = window.__plugins || []).push({
  id: 'palette',
  icon: 'command.square',
  en: {
    'nav.palette': 'Command palette', 'page.palette': 'Command palette',
    'page.palette.sub': 'One shortcut, then type: open a tab or a page, turn a feature on or off, go to the agent that waits, test a connection, add a reminder.',
    'target.palette': 'the command palette shortcut',
    'palette.enable.title': 'Use the command palette', 'palette.enable.desc': 'Opens a small window with a field to type in. Nothing is sent anywhere and nothing is kept.',
    'palette.shortcut.title': 'Shortcut', 'palette.shortcut.desc': 'Works in any app. Press it again, or Escape, to close the window.',
    'sec.palette.links': 'Addresses',
    'palette.links.title': 'figghost:// addresses', 'palette.links.desc': 'Shortcuts, scripts and links can open Figghost with an address. Only these do anything, and none changes a setting or sends data: figghost://palette, figghost://page/network, figghost://tab/pomodoro, figghost://agents/jump and figghost://remind?text=call%20mom%20tomorrow%209h.',
    'palette.placeholder': 'Type a command...', 'palette.none': 'Nothing matches.',
    'palette.group.tab': 'Notch tab', 'palette.group.page': 'Settings', 'palette.group.feature': 'Feature', 'palette.group.agents': 'Agents', 'palette.group.network': 'Network', 'palette.group.reminder': 'Reminder',
    'palette.tab': 'Open the {name} tab', 'palette.page': 'Open {name} settings',
    'palette.on': 'Turn on {name}', 'palette.off': 'Turn off {name}',
    'palette.jump': 'Go to the agent that waits longest', 'palette.jump.hint': 'Brings its terminal to the front',
    'palette.speed': 'Test the speed of {name}', 'palette.export': 'Export the network history of 7 days as CSV',
    'palette.remind': 'Add a reminder: {text}', 'palette.remind.hint': 'Say when too: tomorrow 9h, friday 5pm, in 30 minutes',
    'palette.added': 'Reminder added: {text}',
  },
  vi: {
    'nav.palette': 'Bảng lệnh', 'page.palette': 'Bảng lệnh',
    'page.palette.sub': 'Một phím tắt rồi gõ: mở một tab hay một trang, bật tắt tính năng, tới agent đang chờ, đo một đường mạng, thêm nhắc việc.',
    'target.palette': 'phím tắt Bảng lệnh',
    'palette.enable.title': 'Dùng bảng lệnh', 'palette.enable.desc': 'Mở một cửa sổ nhỏ có ô để gõ. Không gửi gì đi đâu và không lưu gì.',
    'palette.shortcut.title': 'Phím tắt', 'palette.shortcut.desc': 'Dùng ở mọi ứng dụng. Nhấn lại, hoặc Escape, để đóng cửa sổ.',
    'sec.palette.links': 'Địa chỉ',
    'palette.links.title': 'Địa chỉ figghost://', 'palette.links.desc': 'Phím tắt, script và liên kết có thể mở Figghost bằng một địa chỉ. Chỉ những địa chỉ này có tác dụng, và không cái nào đổi cài đặt hay gửi dữ liệu: figghost://palette, figghost://page/network, figghost://tab/pomodoro, figghost://agents/jump và figghost://remind?text=goi%20me%20ngay%20mai%209h.',
    'palette.placeholder': 'Gõ một lệnh...', 'palette.none': 'Không có gì khớp.',
    'palette.group.tab': 'Tab notch', 'palette.group.page': 'Cài đặt', 'palette.group.feature': 'Tính năng', 'palette.group.agents': 'Agent', 'palette.group.network': 'Mạng', 'palette.group.reminder': 'Nhắc việc',
    'palette.tab': 'Mở tab {name}', 'palette.page': 'Mở cài đặt {name}',
    'palette.on': 'Bật {name}', 'palette.off': 'Tắt {name}',
    'palette.jump': 'Tới agent đã chờ lâu nhất', 'palette.jump.hint': 'Đưa terminal của nó lên trước',
    'palette.speed': 'Đo tốc độ của {name}', 'palette.export': 'Xuất lịch sử mạng 7 ngày ra CSV',
    'palette.remind': 'Thêm nhắc việc: {text}', 'palette.remind.hint': 'Nói cả lúc nào: mai 9h, thứ 6 5 chiều, sau 30 phút',
    'palette.added': 'Đã thêm nhắc việc: {text}',
  },

  /** The commands the app has now, each with the words that find it. */
  commands() {
    const list = [];
    const act = (name) => () => send({ t: 'action', name });
    for (const [id, title] of (S.notch.pages || []).filter((p) => p.on).map((p) => [p.id, p.title])) {
      list.push({ id: 'tab:' + id, group: 'tab', title: t('palette.tab', { name: title }), run: act('palette_tab:' + id) });
    }
    for (const id of pageOrder()) {
      const name = t('nav.' + id);
      list.push({ id: 'page:' + id, group: 'page', title: t('palette.page', { name }), run: act('palette_page:' + id) });
      if (S[id] && typeof S[id].enabled === 'boolean') {
        const on = S[id].enabled;
        list.push({ id: 'toggle:' + id, group: 'feature', title: t(on ? 'palette.off' : 'palette.on', { name }), run: () => send({ t: 'set', path: id + '.enabled', value: !on }) });
      }
    }
    list.push({ id: 'agents:jump', group: 'agents', title: t('palette.jump'), hint: t('palette.jump.hint'), run: act('palette_jump') });
    if (S.network && S.network.enabled) {
      for (const l of S.network.links || []) {
        list.push({ id: 'network:speed:' + l.name, group: 'network', title: t('palette.speed', { name: l.label || l.name }), run: act('network_speed_test:' + l.name) });
      }
      list.push({ id: 'network:export', group: 'network', title: t('palette.export'), run: act('network_history_export:csv:7:day') });
    }
    return list;
  },

  /** The commands that say every word of `query`, the best first; with no words, all of them in the order given. */
  rank(query, list) {
    const words = folded(query).split(/\s+/).filter(Boolean);
    if (!words.length) return list;
    const scored = [];
    for (const c of list) {
      const title = folded(c.title);
      const group = folded(t('palette.group.' + c.group));
      let score = 0;
      let all = true;
      for (const w of words) {
        const at = title.indexOf(w);
        if (at === 0) score += 0;
        else if (at > 0 && title[at - 1] === ' ') score += 1;
        else if (at > 0) score += 3;
        else if (group.includes(w)) score += 4;
        else { all = false; break; }
      }
      if (all) scored.push([score, c]);
    }
    // The sort is stable: among equals the order the commands were made in stays.
    return scored.sort((a, b) => a[0] - b[0]).map(([, c]) => c);
  },

  onMessage(message) {
    if (message.t !== 'palette_result') return;
    const text = message.ok ? t('palette.added', { text: message.text }) : message.text;
    toast(text);
    if (message.ok) setTimeout(() => send({ t: 'action', name: 'palette_close' }), 900);
  },

  build() {
    const on = () => S.palette.enabled;
    return [
      card(toggleRow('palette.enabled', t('palette.enable.title'), t('palette.enable.desc')),
        row(t('palette.shortcut.title'), t('palette.shortcut.desc'), hotkeyButton('palette', { label: t('palette.shortcut.title') }), { when: on })),
      sectionTitle(t('sec.palette.links')),
      card(infoRow(t('palette.links.title'), t('palette.links.desc'))),
    ];
  },

  // The window itself: a field, and the commands that say what was typed.
  canvas() {
    const plugin = PLUGINS.find((p) => p.id === 'palette');
    const input = h('input', { type: 'text', class: 'text-field palette-input', placeholder: t('palette.placeholder'), 'aria-label': t('palette.placeholder'), spellcheck: 'false', autocomplete: 'off' });
    const box = h('div', { class: 'palette-list', role: 'listbox' });
    let shown = [];
    let at = 0;
    const entries = () => {
      const found = plugin.rank(input.value, plugin.commands());
      const text = input.value.trim();
      // Anything typed can be a reminder; it comes last, so that a command that says the same words comes first.
      if (text.length > 1 && S.calendar && S.calendar.enabled) {
        found.push({ id: 'remind', group: 'reminder', title: t('palette.remind', { text }), hint: t('palette.remind.hint'), run: () => send({ t: 'action', name: 'palette_remind:' + text }), stay: true });
      }
      return found;
    };
    const draw = () => {
      shown = entries().slice(0, 60);
      at = Math.min(at, Math.max(0, shown.length - 1));
      box.replaceChildren(...(shown.length ? shown.map((c, i) => h('button', {
        class: 'palette-item', type: 'button', role: 'option', 'aria-selected': String(i === at), 'data-id': c.id,
        onmousemove: () => { if (at !== i) { at = i; mark(); } },
        onclick: () => choose(c),
      }, h('span', { class: 'palette-title', text: c.title }), h('span', { class: 'palette-group', text: c.hint || t('palette.group.' + c.group) })))
        : [h('div', { class: 'row-desc palette-none', text: t('palette.none') })]));
      mark();
    };
    const mark = () => {
      [...box.children].forEach((el, i) => el.setAttribute('aria-selected', String(i === at)));
      const el = box.children[at];
      if (el && el.scrollIntoView) el.scrollIntoView({ block: 'nearest' });
    };
    const choose = (c) => {
      c.run();
      // A command ends the window, except the reminder, which says whether it worked first.
      if (!c.stay) send({ t: 'action', name: 'palette_close' });
    };
    input.addEventListener('input', () => { at = 0; draw(); });
    window.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') { e.preventDefault(); send({ t: 'action', name: 'palette_close' }); }
      else if (e.key === 'ArrowDown') { e.preventDefault(); at = Math.min(shown.length - 1, at + 1); mark(); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); at = Math.max(0, at - 1); mark(); }
      else if (e.key === 'Enter' && shown[at]) { e.preventDefault(); choose(shown[at]); }
    });
    reg(() => { input.placeholder = t('palette.placeholder'); draw(); });
    setTimeout(() => input.focus(), 50);
    return h('div', { class: 'palette' }, h('div', { class: 'palette-head', 'data-drag': '' }, input), box);
  },

  selftest({ expect, S }) {
    const plugin = PLUGINS.find((p) => p.id === 'palette');
    const list = plugin.commands();
    const ids = list.map((c) => c.id);
    expect(ids.some((i) => i.startsWith('tab:')) && ids.some((i) => i.startsWith('page:network')) && ids.includes('agents:jump'), 'palette: tabs, pages and the agent jump are commands: ' + ids.slice(0, 6).join());
    const toggle = list.find((c) => c.id === 'toggle:network');
    expect(toggle && toggle.title.includes(t(S.network.enabled ? 'palette.off' : 'palette.on', { name: t('nav.network') })), 'palette: a feature is offered to be turned on when it is off and off when it is on');
    expect(plugin.rank('', list).length === list.length, 'palette: no words, every command');
    const fake = [{ title: 'Open the Notes tab', group: 'tab' }, { title: 'Turn on Network', group: 'feature' }, { title: 'Open Network settings', group: 'page' }, { title: 'Mở cài đặt Cuộn chuột', group: 'page' }];
    expect(plugin.rank('net', fake).map((c) => c.title).join('|') === 'Turn on Network|Open Network settings', 'palette: a word is found where a title has it, best match first');
    expect(plugin.rank('cuon chuot', fake).length === 1 && plugin.rank('MỞ cai', fake).length === 1, 'palette: Vietnamese is found without its marks and in any case');
    expect(plugin.rank('open net', fake).map((c) => c.title).join() === 'Open Network settings', 'palette: every word has to match');
    expect(plugin.rank('zzz', fake).length === 0, 'palette: nothing matches nonsense');
    expect(plugin.rank('feature', fake).length === 1, 'palette: the name of the group finds its commands too');
  },
});
