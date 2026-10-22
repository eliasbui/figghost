'use strict';
// The window of the Windows and Linux beta. Everything it shows comes from the app (fig.receive) and everything it
// changes goes back as a message (send); nothing is kept here that the app does not also have.

const STRINGS = {
  en: {
    'page.clipboard': 'Clipboard',
    'page.notes': 'Notes',
    'page.agents': 'AI agents',
    'page.settings': 'Settings',
    'page.about': 'About',
    'beta': 'Beta',
    'foot': 'Beta for {os}. Some features of the macOS app are not here yet.',
    'clip.sub': 'What you copied, newest first. Click an item to put it back on the clipboard, then paste it with Ctrl+V.',
    'clip.search': 'Search. Filters: @link @code @text @image @file @pin',
    'clip.empty': 'Nothing copied yet.',
    'clip.none': 'Nothing matches.',
    'clip.clear': 'Clear history',
    'clip.clear.ask': 'Delete every item that is not pinned?',
    'clip.count': '{n} items',
    'clip.pin': 'Pin',
    'clip.unpin': 'Unpin',
    'clip.delete': 'Delete',
    'clip.hidden': 'Hidden: looks like a password or a key',
    'clip.copied': 'Copied. Paste it with Ctrl+V.',
    'clip.files': '{n} files',
    'clip.shortcut': 'Shortcut: {key}',
    'kind.text': 'Text', 'kind.link': 'Link', 'kind.code': 'Code', 'kind.image': 'Image', 'kind.file': 'File',
    'notes.sub': 'A scratchpad. Each note is a plain text file in the data folder; it is saved a moment after you stop typing.',
    'notes.new': 'New note',
    'notes.delete': 'Delete',
    'notes.delete.ask': 'Move this note to the trash?',
    'notes.empty': 'No notes yet.',
    'notes.untitled': 'New note',
    'notes.chars': '{n} characters',
    'agents.sub': 'See what Claude Code, Codex and pi are doing, and answer their permission requests here.',
    'agents.enable': 'Follow coding agents',
    'agents.enable.hint': 'Listens on a local socket for the hooks the agents run.',
    'agents.hooks': 'Hooks',
    'agents.install': 'Install',
    'agents.remove': 'Remove',
    'agents.installed': 'Installed',
    'agents.missing': 'Not installed',
    'agents.waiting': 'Waiting for you',
    'agents.sessions': 'Sessions',
    'agents.none': 'No session yet. Start an agent after installing its hooks.',
    'agents.allow': 'Allow',
    'agents.deny': 'Deny',
    'agents.terminal': 'Answer in the terminal',
    'agents.wants': '{agent} wants to use {tool}',
    'agents.asks': '{agent} asks',
    'agents.wait': 'Wait for an answer (seconds)',
    'agents.windows': 'On Windows, Claude Code runs its hooks in Git Bash, which must be installed. pi is not supported on Windows yet.',
    'state.waiting': 'Waiting', 'state.working': 'Working', 'state.failed': 'Failed', 'state.done': 'Done', 'state.idle': 'Idle',
    'settings.sub': 'How the beta behaves on this computer.',
    'settings.general': 'General',
    'settings.language': 'Language',
    'settings.theme': 'Appearance',
    'settings.autostart': 'Start at login',
    'settings.autostart.na': 'Not available on this system.',
    'settings.clipboard': 'Clipboard',
    'settings.hotkey': 'Shortcut for the history',
    'settings.hotkey.hint': 'For example ctrl+alt+KeyV or super+shift+KeyC. Leave empty for none.',
    'settings.limit': 'Items kept',
    'settings.secrets': 'Passwords and keys',
    'settings.mcp': 'Let AI assistants search the history (MCP)',
    'settings.mcp.hint': 'Add this command to your assistant as an MCP server:',
    'settings.data': 'Data folder',
    'settings.open': 'Open',
    'settings.save': 'Save',
    'opt.auto': 'Same as the system', 'opt.en': 'English', 'opt.vi': 'Tiếng Việt',
    'opt.light': 'Light', 'opt.dark': 'Dark',
    'opt.keep': 'Keep them like any copy', 'opt.mask': 'Hide them and delete after an hour', 'opt.skip': 'Never keep them',
    'about.sub': 'Figghost for Windows and Linux is a beta.',
    'about.version': 'Version',
    'about.here': 'In this beta',
    'about.here.list': 'Tray icon and menu|Clipboard history with search, pins and a global shortcut|Notes|AI agents: sessions, permission requests, hooks|MCP server for AI assistants|Start at login|English and Vietnamese',
    'about.later': 'Not in this beta yet',
    'about.later.list': 'The notch panel and its tabs|Smooth scrolling|Window snapping|Per-app sound|Screenshots and recording|Smart Select, the drag wheel, Vietnamese typing and the other macOS features',
    'about.limits': 'Known limits',
    'about.limits.list': 'Pictures are kept on Windows only; on Linux the history keeps text and files.|The clipboard is read twice a second, not on every change.|On Linux, copies marked secret by a password manager are not told apart; the passwords setting is the guard.|Under Wayland the global shortcut and the tray depend on the desktop; GNOME needs the AppIndicator extension for the tray.',
    'about.wayland': 'You are on Wayland. If the shortcut does nothing, bind a key in your desktop settings to run Figghost again: a second start shows this window.',
    'error': 'Could not do that: {e}',
    'ago.now': 'just now', 'ago.m': '{n} min ago', 'ago.h': '{n} h ago', 'ago.d': '{n} d ago',
  },
  vi: {
    'page.clipboard': 'Clipboard',
    'page.notes': 'Ghi chú',
    'page.agents': 'AI agents',
    'page.settings': 'Cài đặt',
    'page.about': 'Giới thiệu',
    'beta': 'Beta',
    'foot': 'Bản beta cho {os}. Một số tính năng của bản macOS chưa có.',
    'clip.sub': 'Những gì bạn đã sao chép, mới nhất ở trên. Bấm vào một mục để đưa lại vào clipboard, rồi dán bằng Ctrl+V.',
    'clip.search': 'Tìm kiếm. Bộ lọc: @link @code @text @image @file @pin',
    'clip.empty': 'Chưa sao chép gì.',
    'clip.none': 'Không có mục nào khớp.',
    'clip.clear': 'Xoá lịch sử',
    'clip.clear.ask': 'Xoá mọi mục chưa ghim?',
    'clip.count': '{n} mục',
    'clip.pin': 'Ghim',
    'clip.unpin': 'Bỏ ghim',
    'clip.delete': 'Xoá',
    'clip.hidden': 'Đã ẩn: trông giống mật khẩu hoặc khoá',
    'clip.copied': 'Đã sao chép. Dán bằng Ctrl+V.',
    'clip.files': '{n} tệp',
    'clip.shortcut': 'Phím tắt: {key}',
    'kind.text': 'Chữ', 'kind.link': 'Liên kết', 'kind.code': 'Mã', 'kind.image': 'Ảnh', 'kind.file': 'Tệp',
    'notes.sub': 'Sổ nháp. Mỗi ghi chú là một tệp văn bản trong thư mục dữ liệu; được lưu ngay sau khi bạn ngừng gõ.',
    'notes.new': 'Ghi chú mới',
    'notes.delete': 'Xoá',
    'notes.delete.ask': 'Chuyển ghi chú này vào thùng rác?',
    'notes.empty': 'Chưa có ghi chú.',
    'notes.untitled': 'Ghi chú mới',
    'notes.chars': '{n} ký tự',
    'agents.sub': 'Xem Claude Code, Codex và pi đang làm gì, và trả lời yêu cầu cấp quyền của chúng ngay tại đây.',
    'agents.enable': 'Theo dõi coding agents',
    'agents.enable.hint': 'Nghe trên một socket cục bộ các hook mà agent chạy.',
    'agents.hooks': 'Hook',
    'agents.install': 'Cài',
    'agents.remove': 'Gỡ',
    'agents.installed': 'Đã cài',
    'agents.missing': 'Chưa cài',
    'agents.waiting': 'Đang chờ bạn',
    'agents.sessions': 'Phiên làm việc',
    'agents.none': 'Chưa có phiên nào. Hãy chạy một agent sau khi cài hook.',
    'agents.allow': 'Cho phép',
    'agents.deny': 'Từ chối',
    'agents.terminal': 'Trả lời trong terminal',
    'agents.wants': '{agent} muốn dùng {tool}',
    'agents.asks': '{agent} hỏi',
    'agents.wait': 'Chờ câu trả lời (giây)',
    'agents.windows': 'Trên Windows, Claude Code chạy hook bằng Git Bash, cần cài sẵn. pi chưa hỗ trợ trên Windows.',
    'state.waiting': 'Đang chờ', 'state.working': 'Đang làm', 'state.failed': 'Lỗi', 'state.done': 'Xong', 'state.idle': 'Rảnh',
    'settings.sub': 'Cách bản beta hoạt động trên máy này.',
    'settings.general': 'Chung',
    'settings.language': 'Ngôn ngữ',
    'settings.theme': 'Giao diện',
    'settings.autostart': 'Mở khi đăng nhập',
    'settings.autostart.na': 'Hệ thống này không hỗ trợ.',
    'settings.clipboard': 'Clipboard',
    'settings.hotkey': 'Phím tắt mở lịch sử',
    'settings.hotkey.hint': 'Ví dụ ctrl+alt+KeyV hoặc super+shift+KeyC. Để trống nếu không dùng.',
    'settings.limit': 'Số mục giữ lại',
    'settings.secrets': 'Mật khẩu và khoá',
    'settings.mcp': 'Cho trợ lý AI tìm trong lịch sử (MCP)',
    'settings.mcp.hint': 'Thêm lệnh này vào trợ lý của bạn như một MCP server:',
    'settings.data': 'Thư mục dữ liệu',
    'settings.open': 'Mở',
    'settings.save': 'Lưu',
    'opt.auto': 'Theo hệ thống', 'opt.en': 'English', 'opt.vi': 'Tiếng Việt',
    'opt.light': 'Sáng', 'opt.dark': 'Tối',
    'opt.keep': 'Giữ như mọi mục khác', 'opt.mask': 'Ẩn đi và xoá sau một giờ', 'opt.skip': 'Không bao giờ giữ',
    'about.sub': 'Figghost cho Windows và Linux đang là bản beta.',
    'about.version': 'Phiên bản',
    'about.here': 'Có trong bản beta',
    'about.here.list': 'Biểu tượng và menu ở khay hệ thống|Lịch sử clipboard có tìm kiếm, ghim và phím tắt toàn cục|Ghi chú|AI agents: phiên làm việc, yêu cầu cấp quyền, hook|MCP server cho trợ lý AI|Mở khi đăng nhập|Tiếng Anh và tiếng Việt',
    'about.later': 'Chưa có trong bản beta',
    'about.later.list': 'Notch và các tab của nó|Cuộn mượt|Snap cửa sổ|Âm thanh theo từng app|Chụp và quay màn hình|Smart Select, drag wheel, gõ tiếng Việt và các tính năng macOS khác',
    'about.limits': 'Giới hạn đã biết',
    'about.limits.list': 'Ảnh chỉ được lưu trên Windows; trên Linux lịch sử giữ chữ và tệp.|Clipboard được đọc hai lần mỗi giây, không theo từng thay đổi.|Trên Linux, nội dung trình quản lý mật khẩu đánh dấu bí mật không được nhận ra; cài đặt mật khẩu là lớp bảo vệ.|Trên Wayland, phím tắt toàn cục và khay hệ thống tuỳ desktop; GNOME cần extension AppIndicator để hiện khay.',
    'about.wayland': 'Bạn đang dùng Wayland. Nếu phím tắt không chạy, hãy gán một phím trong cài đặt desktop để chạy lại Figghost: lần chạy thứ hai sẽ hiện cửa sổ này.',
    'error': 'Không làm được: {e}',
    'ago.now': 'vừa xong', 'ago.m': '{n} phút trước', 'ago.h': '{n} giờ trước', 'ago.d': '{n} ngày trước',
  },
};

const PAGES = ['clipboard', 'notes', 'agents', 'settings', 'about'];
const OS_NAMES = { windows: 'Windows', linux: 'Linux', macos: 'macOS' };

let state = null;
let page = 'clipboard';
let items = [];
let query = '';
let selected = 0;
let notes = null;
let currentNote = null;
let saveTimer = null;
let searchTimer = null;

function lang() { return state && STRINGS[state.env.lang] ? state.env.lang : 'en'; }

function t(key, values) {
  let text = STRINGS[lang()][key] ?? STRINGS.en[key] ?? key;
  for (const [name, value] of Object.entries(values || {})) text = text.replaceAll('{' + name + '}', value);
  return text;
}

function send(message) { window.ipc.postMessage(JSON.stringify(message)); }

function h(tag, attrs, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs || {})) {
    if (value === undefined || value === null || value === false) continue;
    if (key.startsWith('on')) node.addEventListener(key.slice(2), value);
    else if (key === 'text') node.textContent = value;
    else if (key === 'class') node.className = value;
    else node.setAttribute(key, value === true ? '' : value);
  }
  for (const child of children.flat()) if (child !== null && child !== undefined && child !== false) node.append(child);
  return node;
}

function ago(ms) {
  const seconds = Math.max(0, (Date.now() - ms) / 1000);
  if (seconds < 45) return t('ago.now');
  if (seconds < 3600) return t('ago.m', { n: Math.max(1, Math.round(seconds / 60)) });
  if (seconds < 86400) return t('ago.h', { n: Math.round(seconds / 3600) });
  return t('ago.d', { n: Math.round(seconds / 86400) });
}

function toast(text) {
  const node = document.getElementById('toast');
  node.textContent = text;
  node.classList.add('on');
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => node.classList.remove('on'), 2400);
}

function header(key) {
  return [h('h1', { text: t('page.' + key) }), h('p', { class: 'sub', text: t(key === 'clipboard' ? 'clip.sub' : key + '.sub') })];
}

function toggle(checked, onchange, label) {
  return h('label', { class: 'switch' },
    h('input', { type: 'checkbox', role: 'switch', 'aria-label': label, checked, onchange: e => onchange(e.target.checked) }),
    h('span'));
}

function select(value, options, onchange, label) {
  return h('select', { 'aria-label': label, onchange: e => onchange(e.target.value) },
    options.map(o => h('option', { value: o, selected: o === value, text: t('opt.' + o) })));
}

function set(key, value) { send({ t: 'set', key, value }); }

function list(key) { return h('ul', {}, t(key).split('|').map(line => h('li', { text: line }))); }

function nav() {
  const pages = document.getElementById('pages');
  const waiting = state ? state.agents.approvals.length : 0;
  pages.replaceChildren(...PAGES.map(id => h('li', {},
    h('button', { 'aria-current': id === page ? 'page' : null, onclick: () => go(id) },
      h('span', { text: t('page.' + id) }),
      id === 'agents' && waiting ? h('span', { class: 'badge', text: String(waiting) }) : null))));
  document.getElementById('beta').textContent = t('beta');
  document.getElementById('foot').textContent = state ? t('foot', { os: OS_NAMES[state.env.os] || state.env.os }) : '';
  document.documentElement.lang = lang();
  document.documentElement.dataset.theme = state ? state.config.theme : 'auto';
}

function go(id) {
  if (!PAGES.includes(id)) return;
  page = id;
  if (id === 'notes' && notes === null) send({ t: 'notes' });
  if (id === 'clipboard') send({ t: 'search', q: query });
  render();
}

function render() {
  nav();
  if (!state) return;
  const main = document.getElementById('main');
  const draw = { clipboard: clipboardPage, notes: notesPage, agents: agentsPage, settings: settingsPage, about: aboutPage }[page];
  const focused = document.activeElement && document.activeElement.id;
  main.replaceChildren(...draw().filter(Boolean));
  if (focused) document.getElementById(focused)?.focus();
}

function itemRow(item, index) {
  let body;
  if (item.masked) body = h('div', { class: 'text hint', text: t('clip.hidden') });
  else if (item.kind === 'image' && item.image) body = h('img', { src: 'image/' + item.image, alt: `${item.width} x ${item.height}` });
  else if (item.kind === 'file') {
    const paths = item.preview.split('\n');
    body = h('div', { class: 'text', text: paths.length > 1 ? `${paths[0]}  (${t('clip.files', { n: paths.length })})` : paths[0] });
  } else body = h('div', { class: 'text' + (item.kind === 'code' ? ' code' : ''), text: item.preview });
  return h('div', {
    class: 'row item' + (item.pinned ? ' pinned' : ''),
    role: 'option',
    'aria-selected': index === selected ? 'true' : 'false',
    tabindex: '-1',
    onclick: () => copy(item.id),
  },
  h('span', { class: 'kind', text: t('kind.' + item.kind) }),
  h('div', { class: 'grow' }, body, h('div', { class: 'hint', text: ago(item.used) })),
  h('div', { class: 'tools' },
    h('button', { text: item.pinned ? t('clip.unpin') : t('clip.pin'), onclick: e => { e.stopPropagation(); send({ t: 'pin', id: item.id }); refresh(); } }),
    h('button', { text: t('clip.delete'), onclick: e => { e.stopPropagation(); send({ t: 'delete', id: item.id }); refresh(); } })));
}

function copy(id) {
  send({ t: 'copy', id });
  toast(t('clip.copied'));
}

function refresh() { send({ t: 'search', q: query }); }

function clipboardPage() {
  const search = h('input', {
    id: 'search', type: 'search', placeholder: t('clip.search'), value: query, 'aria-label': t('clip.search'),
    oninput: e => {
      query = e.target.value;
      selected = 0;
      clearTimeout(searchTimer);
      searchTimer = setTimeout(refresh, 120);
    },
    onkeydown: e => {
      if (e.key === 'ArrowDown') { selected = Math.min(items.length - 1, selected + 1); render(); e.preventDefault(); }
      if (e.key === 'ArrowUp') { selected = Math.max(0, selected - 1); render(); e.preventDefault(); }
      if (e.key === 'Enter' && items[selected]) copy(items[selected].id);
    },
  });
  const shortcut = state.config.hotkey && !state.env.hotkey_error
    ? h('span', { class: 'hint', text: t('clip.shortcut', { key: state.config.hotkey }) }) : null;
  const top = h('div', { class: 'row' },
    h('div', { class: 'grow hint', text: t('clip.count', { n: state.env.count }) }),
    shortcut,
    h('button', { class: 'action danger', text: t('clip.clear'), onclick: () => { if (confirm(t('clip.clear.ask'))) { send({ t: 'clear' }); refresh(); } } }));
  const rows = items.length
    ? h('div', { class: 'card', role: 'listbox', 'aria-label': t('page.clipboard') }, items.map(itemRow))
    : h('div', { class: 'empty', text: query ? t('clip.none') : t('clip.empty') });
  return [...header('clipboard'), search, h('div', { class: 'card' }, top), h('h2'), rows];
}

function noteTitle(note) {
  const line = note.text.split('\n').map(l => l.trim()).find(l => l);
  return line ? line.replace(/^#+\s*/, '').slice(0, 60) : t('notes.untitled');
}

function saveNote() {
  clearTimeout(saveTimer);
  saveTimer = null;
  const note = notes && notes.find(n => n.id === currentNote);
  if (note) send({ t: 'note_save', id: note.id, text: note.text });
}

function notesPage() {
  if (notes === null) return header('notes');
  const note = notes.find(n => n.id === currentNote) || notes[0];
  if (note) currentNote = note.id;
  const rows = notes.length
    ? notes.map(n => h('div', {
      class: 'row', role: 'option', 'aria-selected': n.id === currentNote ? 'true' : 'false',
      onclick: () => { saveNote(); currentNote = n.id; render(); },
    }, h('div', { class: 'title', text: noteTitle(n) }), h('div', { class: 'hint', text: ago(n.updated) })))
    : [h('div', { class: 'empty', text: t('notes.empty') })];
  const editor = note
    ? h('div', { class: 'editor' },
      h('textarea', {
        id: 'note', 'aria-label': noteTitle(note), maxlength: '50000',
        oninput: e => {
          note.text = e.target.value;
          note.updated = Date.now();
          document.getElementById('chars').textContent = t('notes.chars', { n: [...note.text].length });
          clearTimeout(saveTimer);
          saveTimer = setTimeout(saveNote, 600);
        },
        onblur: () => { if (saveTimer) saveNote(); },
      }, note.text),
      h('div', { class: 'row' },
        h('div', { class: 'grow hint', id: 'chars', text: t('notes.chars', { n: [...note.text].length }) }),
        h('button', { class: 'action danger', text: t('notes.delete'), onclick: () => {
          if (!confirm(t('notes.delete.ask'))) return;
          clearTimeout(saveTimer);
          send({ t: 'note_delete', id: note.id });
          currentNote = null;
        } })))
    : h('div', { class: 'editor' });
  const add = h('button', { class: 'action primary', text: t('notes.new'), onclick: () => {
    saveNote();
    const blank = notes.find(n => !n.text.trim());
    const id = blank ? blank.id : Math.max(Date.now(), ...notes.map(n => n.id + 1));
    if (!blank) notes.unshift({ id, text: '', updated: Date.now() });
    currentNote = id;
    render();
    document.getElementById('note')?.focus();
  } });
  return [...header('notes'), h('p', {}, add), h('div', { class: 'notes' },
    h('div', { class: 'card list', role: 'listbox' }, rows), editor)];
}

function agentsPage() {
  const a = state.agents;
  const cfg = state.config.agents;
  const out = [...header('agents')];
  if (state.env.os === 'windows') out.push(h('div', { class: 'notice', text: t('agents.windows') }));
  if (state.env.agents_error) out.push(h('div', { class: 'notice', text: t('error', { e: state.env.agents_error }) }));
  out.push(h('div', { class: 'card' },
    h('div', { class: 'row' },
      h('div', { class: 'grow' }, h('div', { text: t('agents.enable') }), h('div', { class: 'hint', text: t('agents.enable.hint') })),
      toggle(cfg.enabled, v => set('agents.enabled', v), t('agents.enable'))),
    h('div', { class: 'row' },
      h('div', { class: 'grow', text: t('agents.wait') }),
      h('input', { type: 'number', min: '5', max: '100', value: String(cfg.approval_wait_seconds), 'aria-label': t('agents.wait'),
        onchange: e => set('agents.approval_wait_seconds', Number(e.target.value)) }))));
  out.push(h('h2', { text: t('agents.hooks') }), h('div', { class: 'card' }, a.agents.map(agent => h('div', { class: 'row' },
    h('div', { class: 'grow' }, h('div', { text: agent.name }), h('div', { class: 'hint', text: agent.installed ? t('agents.installed') : t('agents.missing') })),
    h('button', { class: 'action', text: agent.installed ? t('agents.remove') : t('agents.install'),
      onclick: () => send({ t: agent.installed ? 'agent_remove' : 'agent_install', agent: agent.id }) })))));
  if (a.approvals.length) {
    out.push(h('h2', { text: t('agents.waiting') }), h('div', { class: 'card' }, a.approvals.map(q => h('div', { class: 'row' },
      h('div', { class: 'grow' },
        h('div', { text: q.question ? t('agents.asks', { agent: q.agent }) : t('agents.wants', { agent: q.agent, tool: q.tool }) }),
        h('div', { class: 'mono', text: q.question || q.detail }),
        h('div', { class: 'hint', text: `${q.project}  -  ${ago(q.requested)}` })),
      q.question
        ? h('button', { class: 'action', text: t('agents.terminal'), onclick: () => send({ t: 'answer', id: q.id, allow: false }) })
        : h('div', { class: 'tools' },
          h('button', { class: 'action primary', text: t('agents.allow'), onclick: () => send({ t: 'answer', id: q.id, allow: true }) }),
          h('button', { class: 'action danger', text: t('agents.deny'), onclick: () => send({ t: 'answer', id: q.id, allow: false }) }))))));
  }
  out.push(h('h2', { text: t('agents.sessions') }), a.sessions.length
    ? h('div', { class: 'card' }, a.sessions.map(s => h('div', { class: 'row' },
      h('div', { class: 'grow' }, h('div', { text: `${s.agent}  -  ${s.project}` }), h('div', { class: 'hint', text: s.note || ago(s.updated) })),
      h('span', { class: 'state ' + s.state, text: t('state.' + s.state) }))))
    : h('div', { class: 'empty', text: t('agents.none') }));
  return out;
}

function settingsPage() {
  const c = state.config;
  const env = state.env;
  const hotkey = h('input', { id: 'hotkey', type: 'text', value: c.hotkey, 'aria-label': t('settings.hotkey'), spellcheck: 'false' });
  const limit = h('input', { type: 'number', min: '10', max: '100000', value: String(c.history_limit), 'aria-label': t('settings.limit'),
    onchange: e => set('history_limit', Number(e.target.value)) });
  return [...header('settings'),
    h('h2', { text: t('settings.general') }),
    h('div', { class: 'card' },
      h('div', { class: 'row' }, h('div', { class: 'grow', text: t('settings.language') }),
        select(c.language, ['auto', 'en', 'vi'], v => set('language', v), t('settings.language'))),
      h('div', { class: 'row' }, h('div', { class: 'grow', text: t('settings.theme') }),
        select(c.theme, ['auto', 'light', 'dark'], v => set('theme', v), t('settings.theme'))),
      h('div', { class: 'row' },
        h('div', { class: 'grow' }, h('div', { text: t('settings.autostart') }),
          env.autostart === null ? h('div', { class: 'hint', text: t('settings.autostart.na') }) : null),
        env.autostart === null ? null : toggle(env.autostart, v => send({ t: 'autostart', value: v }), t('settings.autostart')))),
    h('h2', { text: t('settings.clipboard') }),
    h('div', { class: 'card' },
      h('div', { class: 'row' },
        h('div', { class: 'grow' }, h('div', { text: t('settings.hotkey') }),
          h('div', { class: 'hint', text: env.hotkey_error ? t('error', { e: env.hotkey_error }) : t('settings.hotkey.hint') })),
        hotkey,
        h('button', { class: 'action', text: t('settings.save'), onclick: () => set('hotkey', hotkey.value.trim()) })),
      h('div', { class: 'row' }, h('div', { class: 'grow', text: t('settings.limit') }), limit),
      h('div', { class: 'row' }, h('div', { class: 'grow', text: t('settings.secrets') }),
        select(c.secrets, ['keep', 'mask', 'skip'], v => set('secrets', v), t('settings.secrets'))),
      h('div', { class: 'row' },
        h('div', { class: 'grow' }, h('div', { text: t('settings.mcp') }),
          c.mcp.enabled ? h('div', { class: 'hint', text: t('settings.mcp.hint') }) : null,
          c.mcp.enabled ? h('div', { class: 'mono', text: `"${env.exe}" --mcp` }) : null),
        toggle(c.mcp.enabled, v => set('mcp.enabled', v), t('settings.mcp')))),
    h('h2'),
    h('div', { class: 'card' },
      h('div', { class: 'row' },
        h('div', { class: 'grow' }, h('div', { text: t('settings.data') }), h('div', { class: 'hint mono', text: env.data_dir })),
        h('button', { class: 'action', text: t('settings.open'), onclick: () => send({ t: 'open_data' }) })))];
}

function aboutPage() {
  const env = state.env;
  return [...header('about'),
    h('div', { class: 'notice' }, h('strong', { text: t('beta') + '. ' }), t('foot', { os: OS_NAMES[env.os] || env.os })),
    env.wayland ? h('div', { class: 'notice', text: t('about.wayland') }) : null,
    h('div', { class: 'card' }, h('div', { class: 'row' }, h('div', { class: 'grow', text: t('about.version') }),
      h('span', { text: `${env.version} beta, ${OS_NAMES[env.os] || env.os}` }))),
    h('h2', { text: t('about.here') }), list('about.here.list'),
    h('h2', { text: t('about.later') }), list('about.later.list'),
    h('h2', { text: t('about.limits') }), list('about.limits.list')];
}

window.fig = {
  receive(message) {
    switch (message.t) {
      case 'state':
        state = message;
        render();
        break;
      case 'items':
        if (message.q !== query) return;
        items = message.items;
        selected = Math.min(selected, Math.max(0, items.length - 1));
        if (page === 'clipboard') render();
        break;
      case 'changed':
        if (state) state.env.count += 1;
        if (page === 'clipboard') refresh();
        break;
      case 'notes':
        notes = message.notes;
        if (page === 'notes') render();
        break;
      case 'agents':
        if (!state) return;
        state.agents = message.agents;
        if (page === 'agents') render(); else nav();
        break;
      case 'go':
        go(message.page);
        break;
      case 'focus_search':
        if (page === 'clipboard') document.getElementById('search')?.focus();
        break;
      case 'error':
        toast(t('error', { e: message.message }));
        break;
    }
  },
};

document.addEventListener('keydown', e => {
  if (e.key === 'Escape' && page === 'clipboard' && query) {
    query = '';
    refresh();
    render();
  }
});

send({ t: 'ready' });
send({ t: 'search', q: '' });
