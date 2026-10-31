// Network: what each app moves and through which connection, and (beta) wires that choose the connection of an app,
// its speed and its place behind the apps wired before it. The diagram is drawn by `canvas()` in a window of its own.
(window.__plugins = window.__plugins || []).push({
  id: 'network',
  icon: 'network',
  en: {
    'nav.network': 'Network', 'page.network': 'Network',
    'page.network.sub': 'See which app uses which connection and how much, and wire an app to Wi-Fi, Ethernet or both.',
    'network.enable.title': 'Watch the network', 'network.enable.desc': 'Reads what each app sends and receives, once a second and only while this page is open. Nothing leaves the Mac.',
    'network.open': 'Open the diagram', 'network.open.desc': 'Opens in a window of its own: each connection is a circle and each app a box you can move anywhere. Wire one connection to another to combine them.',
    'sec.network.summary': 'Now',
    'network.total': 'All apps', 'network.apps.none': 'No app is using the network yet.',
    'network.col.app': 'App', 'network.col.down': 'Down', 'network.col.up': 'Up', 'network.col.total': 'Since opened', 'network.col.via': 'Through',
    'sec.network.control': 'Wiring (Beta)',
    'network.control.title': 'Apply the wires', 'network.control.desc': 'Points the proxy settings of macOS at Figghost, so each app that follows them (browsers and most apps) leaves by its wire, within its limit. Apps that ignore those settings, such as many games and command line tools, are only watched. macOS may ask for an administrator password. Turned off, every setting goes back as it was.',
    'network.status.off': 'Off', 'network.status.starting': 'Changing the settings of macOS...', 'network.status.on': 'On: apps that follow the proxy settings go through Figghost', 'network.status.denied': 'The password was not given, so nothing changed. Turn the switch off and on to ask again.', 'network.status.failed': 'The settings of macOS could not be changed.',
    'network.paused': 'A VPN carries everything, so every app takes the usual way and no wire is applied.',
    'network.restore': 'Put back the proxy settings', 'network.restore.desc': 'If websites stop opening after Figghost quit unexpectedly, this puts the settings of macOS back as they were.',
    'network.hub.merge': 'Combined', 'network.hub.merge.desc': 'Every connection at once: each new connection of the app takes the next one', 'network.hub.system': 'Automatic', 'network.hub.system.desc': 'The way macOS chooses',
    'network.links': 'Connections', 'network.meet': 'Single or combined', 'network.apps': 'Apps',
    'network.link.primary': 'macOS uses this one', 'network.links.none': 'No connection with an address.',
    'network.watching': 'Wires are kept now and take effect once Apply the wires is on in Settings. An app without a wire goes the way macOS chooses, drawn dashed.',
    'network.drag': 'Drag from the dot of a connection to an app, or from an app to a connection. Wire Wi-Fi to Ethernet to combine them: an app wired to the big circle uses both, to a small one only that one. Drag a box or a circle to move it.',
    'network.merge.split': 'Separate', 'network.link.away': 'Not connected', 'network.app.idle': 'Not using the network now', 'network.hub.merge.all': 'Combined (every connection)',
    'network.card.target': 'Leaves by', 'network.card.priority': 'Priority', 'network.card.down': 'Most down (KB/s)', 'network.card.up': 'Most up (KB/s)', 'network.card.limit.none': 'No limit',
    'network.card.save': 'Save', 'network.card.unwire': 'Remove the wire', 'network.card.held': 'Held to {speed} for the apps above it',
    'network.priority.high': 'Main', 'network.priority.normal': 'Normal', 'network.priority.low': 'Below',
    'network.close': 'Close',
    'sec.network.tether': 'Share with a phone (Beta)',
    'network.tether.title': 'Android phone on USB', 'network.tether.desc': 'Gives the phone this Mac\'s connection over the cable, through gnirehtet. Turn on USB debugging on the phone, plug it in, then start.',
    'network.tether.missing': 'Needs gnirehtet and adb: brew install gnirehtet android-platform-tools',
    'network.tether.none': 'No phone found', 'network.tether.phones': 'Phones: {list}', 'network.tether.unauthorized': 'A phone waits: allow this Mac on its screen',
    'network.tether.start': 'Start sharing', 'network.tether.stop': 'Stop', 'network.tether.running': 'Sharing',
    'network.tether.iphone': 'An iPhone cannot take a Mac\'s connection over USB this way. Use Internet Sharing in System Settings, General, Sharing.',
  },
  vi: {
    'nav.network': 'Mạng', 'page.network': 'Mạng',
    'page.network.sub': 'Xem app nào đang dùng đường mạng nào, dùng bao nhiêu, và nối app vào Wi-Fi, Ethernet hay cả hai.',
    'network.enable.title': 'Theo dõi mạng', 'network.enable.desc': 'Đọc lượng dữ liệu mỗi app gửi và nhận, mỗi giây một lần và chỉ khi trang này đang mở. Không có gì rời khỏi máy.',
    'network.open': 'Mở sơ đồ', 'network.open.desc': 'Mở trong một cửa sổ riêng: mỗi đường mạng là một vòng tròn, mỗi app là một ô có thể kéo đi bất cứ đâu. Nối đường mạng này sang đường mạng kia để gộp chúng.',
    'sec.network.summary': 'Hiện tại',
    'network.total': 'Mọi app', 'network.apps.none': 'Chưa có app nào dùng mạng.',
    'network.col.app': 'App', 'network.col.down': 'Tải về', 'network.col.up': 'Tải lên', 'network.col.total': 'Từ lúc mở', 'network.col.via': 'Qua',
    'sec.network.control': 'Nối dây (Beta)',
    'network.control.title': 'Áp dụng các dây', 'network.control.desc': 'Trỏ cài đặt proxy của macOS về Figghost, để app nào theo cài đặt đó (trình duyệt và phần lớn app) đi đúng dây của nó, trong giới hạn của nó. App bỏ qua cài đặt này, như nhiều game và công cụ dòng lệnh, chỉ được theo dõi. macOS có thể hỏi mật khẩu quản trị. Khi tắt, mọi cài đặt trở lại như cũ.',
    'network.status.off': 'Đang tắt', 'network.status.starting': 'Đang đổi cài đặt của macOS...', 'network.status.on': 'Đang bật: app theo cài đặt proxy đi qua Figghost', 'network.status.denied': 'Chưa nhập mật khẩu nên chưa đổi gì. Tắt rồi bật lại công tắc để hỏi lần nữa.', 'network.status.failed': 'Không đổi được cài đặt của macOS.',
    'network.paused': 'VPN đang chở toàn bộ lưu lượng, nên mọi app đi đường thường và chưa dây nào được áp dụng.',
    'network.restore': 'Trả lại cài đặt proxy', 'network.restore.desc': 'Nếu web không mở được sau khi Figghost bị thoát đột ngột, nút này đưa cài đặt của macOS về như cũ.',
    'network.hub.merge': 'Gộp', 'network.hub.merge.desc': 'Dùng mọi đường cùng lúc: mỗi kết nối mới của app đi một đường kế tiếp', 'network.hub.system': 'Tự động', 'network.hub.system.desc': 'Đường macOS tự chọn',
    'network.links': 'Đường mạng', 'network.meet': 'Đơn hay gộp', 'network.apps': 'App',
    'network.link.primary': 'macOS đang dùng đường này', 'network.links.none': 'Chưa có đường mạng nào có địa chỉ.',
    'network.watching': 'Dây được lưu ngay và có hiệu lực khi bật Áp dụng các dây trong Cài đặt. App chưa nối dây đi đường macOS tự chọn, vẽ bằng nét đứt.',
    'network.drag': 'Kéo từ chấm của một đường mạng sang một app, hoặc từ app sang đường mạng. Nối Wi-Fi sang Ethernet để gộp: app nối vào vòng tròn lớn dùng cả hai, nối vào vòng tròn nhỏ chỉ dùng mạng đó. Kéo ô hoặc vòng tròn để di chuyển.',
    'network.merge.split': 'Tách ra', 'network.link.away': 'Chưa kết nối', 'network.app.idle': 'Hiện không dùng mạng', 'network.hub.merge.all': 'Gộp (mọi đường mạng)',
    'network.card.target': 'Đi qua', 'network.card.priority': 'Ưu tiên', 'network.card.down': 'Tải về tối đa (KB/s)', 'network.card.up': 'Tải lên tối đa (KB/s)', 'network.card.limit.none': 'Không giới hạn',
    'network.card.save': 'Lưu', 'network.card.unwire': 'Gỡ dây', 'network.card.held': 'Đang bị giữ ở {speed} để nhường app phía trên',
    'network.priority.high': 'Chính', 'network.priority.normal': 'Thường', 'network.priority.low': 'Xếp sau',
    'network.close': 'Đóng',
    'sec.network.tether': 'Chia mạng cho điện thoại (Beta)',
    'network.tether.title': 'Điện thoại Android cắm USB', 'network.tether.desc': 'Cho điện thoại dùng mạng của Mac qua dây cáp, nhờ gnirehtet. Bật gỡ lỗi USB trên điện thoại, cắm cáp, rồi bấm bắt đầu.',
    'network.tether.missing': 'Cần gnirehtet và adb: brew install gnirehtet android-platform-tools',
    'network.tether.none': 'Chưa thấy điện thoại', 'network.tether.phones': 'Điện thoại: {list}', 'network.tether.unauthorized': 'Có điện thoại đang chờ: cho phép máy Mac này trên màn hình điện thoại',
    'network.tether.start': 'Bắt đầu chia mạng', 'network.tether.stop': 'Dừng', 'network.tether.running': 'Đang chia mạng',
    'network.tether.iphone': 'iPhone không nhận mạng của Mac qua USB theo cách này. Dùng Chia sẻ Internet trong Cài đặt hệ thống, Cài đặt chung, Chia sẻ.',
  },
  build() {
    const N = () => S.network;
    const on = () => N().enabled;
    const when = { when: on };
    const act = (name) => send({ t: 'action', name });
    const { speed, size, linkName } = netHelpers();

    // ---- the summary on the page
    const total = h('div', { class: 'network-total' });
    const table = h('div', { class: 'network-table', role: 'table' });
    reg(() => {
      total.textContent = t('network.total') + ': ' + speed(N().total_down) + ' / ' + speed(N().total_up);
      const apps = N().apps || [];
      if (!apps.length) { table.replaceChildren(h('div', { class: 'row-desc network-empty', text: t('network.apps.none') })); return; }
      const head = h('div', { class: 'network-tr head', role: 'row' },
        ...['app', 'down', 'up', 'total', 'via'].map((k) => h('span', { role: 'columnheader', text: t('network.col.' + k) })));
      const rows = apps.slice(0, 12).map((a) => h('div', { class: 'network-tr', role: 'row' },
        h('span', { class: 'network-app', text: a.name }),
        h('span', { text: speed(a.down) }),
        h('span', { text: speed(a.up) }),
        h('span', { text: size(a.received + a.sent) }),
        h('span', { class: 'network-via', text: (a.interfaces || []).map((i) => linkName(i.name)).join(', ') || '-' })));
      table.replaceChildren(head, ...rows);
    });

    // ---- wiring: the switch, where it stands, the way back
    const status = h('span', { class: 'row-desc network-status' });
    reg(() => {
      const n = N();
      status.textContent = n.paused && n.running ? t('network.paused') : t('network.status.' + (n.status || 'off'));
      status.dataset.state = n.status || 'off';
    });
    const restoreRow = row(t('network.restore'), t('network.restore.desc'), button(t('network.restore'), () => act('network_restore'), { small: true }));
    reg(() => { restoreRow.hidden = !N().saved || (N().control && N().status === 'on'); });

    // ---- the phone
    const phone = h('span', { class: 'row-desc network-status' });
    const start = button(t('network.tether.start'), () => act('network_tether_start'), { small: true, primary: true });
    const stop = button(t('network.tether.stop'), () => act('network_tether_stop'), { small: true });
    reg(() => {
      const tt = N().tether || {};
      const ready = tt.gnirehtet && tt.adb;
      const phones = tt.phones || [];
      phone.textContent = !ready ? t('network.tether.missing')
        : tt.running ? t('network.tether.running')
        : tt.unauthorized ? t('network.tether.unauthorized')
        : phones.length ? t('network.tether.phones', { list: phones.join(', ') }) : t('network.tether.none');
      start.hidden = !ready || tt.running;
      start.disabled = !phones.length;
      stop.hidden = !tt.running;
    });

    return [
      card(toggleRow('network.enabled', t('network.enable.title'), t('network.enable.desc')),
        row(t('network.open'), t('network.open.desc'), button(t('network.open'), () => act('network_canvas_open'), { primary: true, small: true }), when)),
      sectionTitle(t('sec.network.summary')),
      card(row(total, null, null, when), h('div', { class: 'row block network-table-row' }, table)),
      sectionTitle(t('sec.network.control')),
      card(toggleRow('network.control', t('network.control.title'), t('network.control.desc'), when),
        row(status, null, null, when), restoreRow),
      sectionTitle(t('sec.network.tether')),
      card(row(t('network.tether.title'), t('network.tether.desc'), h('div', { class: 'network-phone' }, phone, start, stop), when),
        h('p', { class: 'network-note', text: t('network.tether.iphone') })),
    ];
  },

  // The diagram, in a window of its own (the page told `__canvas` is network): connections as circles, apps as boxes, all
  // moved freely on a dotted grid; one connection wired to another makes the Combined circle around both.
  canvas() {
    const N = () => S.network;
    const send1 = (path, value) => send({ t: 'set', path, value });
    const { speed, linkName } = netHelpers();
    const GRID = 20, D = 96, CHILD = 100, APP_W = 184, APP_H = 54;
    const snap = (v) => Math.max(0, Math.round(v / GRID) * GRID);

    const head = h('header', { class: 'network-canvas-head', 'data-drag': '' });
    const title = h('h1', { text: t('page.network') });
    const totals = h('span', { class: 'network-total-head' });
    const note = h('span', { class: 'network-canvas-note' });
    const close = button(t('network.close'), () => send({ t: 'action', name: 'network_canvas_close' }), { small: true });
    head.append(title, totals, note, close);
    const scroller = h('div', { class: 'network-canvas-scroll' });
    const board = h('div', { class: 'network-canvas-board' });
    const NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(NS, 'svg');
    svg.setAttribute('class', 'network-wires');
    const layer = h('div', { class: 'network-nodes' });
    board.append(svg, layer);
    scroller.append(board);
    const detail = h('div', { class: 'network-card floating', hidden: true });
    const root = h('div', { class: 'network-canvas' }, head, scroller, detail);

    // Where each node is: what the person put there (kept in the settings), else a place of its own on the grid.
    const local = new Map();
    const placed = (id) => local.get(id) || ((N().layout || {})[id] && { x: N().layout[id][0], y: N().layout[id][1] });
    const els = new Map();
    let picked = null, moving = null, wiring = null;
    const told = new Set();

    const model = () => {
      const n = N();
      const live = n.links || [];
      const links = [];
      for (const k of n.known_links || []) links.push({ name: k.name, label: k.label, away: true });
      for (const l of live) {
        const at = links.find((x) => x.name === l.name);
        if (at) Object.assign(at, l, { away: false }); else links.push(Object.assign({}, l, { away: false }));
        // A connection seen is kept on the diagram, also while it is away.
        const known = (n.known_links || []).find((k) => k.name === l.name);
        if ((!known || known.label !== l.label) && !told.has(l.name + l.label)) { told.add(l.name + l.label); send1('network.known_link', { name: l.name, label: l.label }); }
      }
      const merge = (n.merge || []).filter((m) => links.some((l) => l.name === m));
      const apps = [];
      const add = (name) => { if (name && !apps.some((a) => a.name.toLowerCase() === name.toLowerCase())) apps.push({ name, down: 0, up: 0, interfaces: [], idle: true }); };
      for (const name of n.known_apps || []) add(name);
      for (const a of n.apps || []) add(a.name);
      for (const w of n.wires || []) add(w.app);
      for (const a of apps) {
        const now = (n.apps || []).find((x) => x.name.toLowerCase() === a.name.toLowerCase());
        if (now) Object.assign(a, now, { idle: !(now.down || now.up) && !(now.interfaces || []).length });
        a.wire = (n.wires || []).find((w) => w.app.toLowerCase() === a.name.toLowerCase());
      }
      return { links, merge: merge.length >= 2 ? merge : [], apps };
    };

    // Places: the circles down the left, the apps in columns to their right, each until it is moved.
    const layout = (m) => {
      const at = {};
      let y = 40;
      if (m.merge.length) {
        const size = m.merge.length * CHILD + 60;
        at.merge = placed('merge') || { x: 40, y };
        at.merge.size = size;
        y += size + 40;
        m.merge.forEach((name, i) => {
          at['link:' + name] = { x: at.merge.x + size / 2 + (i - (m.merge.length - 1) / 2) * CHILD - D / 2, y: at.merge.y + size / 2 - D / 2, inside: true };
        });
      }
      for (const l of m.links) {
        if (m.merge.includes(l.name)) continue;
        at['link:' + l.name] = placed('link:' + l.name) || { x: 60, y };
        y += D + 50;
      }
      // An app not moved yet takes the next cell of the columns that no box already covers.
      const boxes = [];
      for (const a of m.apps) { const p = placed('app:' + a.name); if (p) { at['app:' + a.name] = p; boxes.push(p); } }
      let cell = 0;
      const clear = (c) => !boxes.some((b) => Math.abs(b.x - c.x) < APP_W + 16 && Math.abs(b.y - c.y) < APP_H + 16);
      for (const a of m.apps) {
        const id = 'app:' + a.name;
        if (at[id]) continue;
        let c;
        do { c = { x: 420 + Math.floor(cell / 9) * 220, y: 40 + (cell % 9) * 80 }; cell++; } while (!clear(c) && cell < 400);
        at[id] = c;
        boxes.push(c);
      }
      return at;
    };

    const node = (id, kind, cls) => {
      let el = els.get(id);
      if (!el) {
        el = h('div', { class: 'network-node' });
        el.dataset.node = id;
        el.dataset.kind = kind;
        const port = h('span', { class: 'network-port', 'aria-hidden': 'true' });
        el.append(h('div', { class: 'network-node-body' }), port);
        port.addEventListener('pointerdown', (e) => startWire(e, id));
        el.addEventListener('pointerdown', (e) => { if (!e.target.closest('.network-port, button')) startMove(e, id); });
        if (kind === 'app') {
          el.tabIndex = 0;
          el.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(id.slice(4)); } });
        }
        layer.append(el);
        els.set(id, el);
      }
      el.className = 'network-node ' + kind + ' ' + cls;
      return el;
    };
    const put = (el, p, size) => {
      el.style.left = p.x + 'px';
      el.style.top = p.y + 'px';
      if (size) { el.style.width = size + 'px'; el.style.height = size + 'px'; }
    };

    let current = null;
    function sync() {
      if (moving || wiring) return;
      const n = N();
      const control = n.control && n.running;
      note.textContent = n.paused ? t('network.paused') : control ? t('network.drag') : t('network.watching') + ' ' + t('network.drag');
      totals.textContent = t('network.total') + ': ' + speed(n.total_down) + ' / ' + speed(n.total_up);
      const m = model();
      const at = layout(m);
      current = { m, at };
      const seen = new Set();
      if (m.merge.length) {
        const el = node('merge', 'merge', '');
        const body = el.firstChild;
        const split = button(t('network.merge.split'), () => send1('network.merge', []), { small: true });
        body.replaceChildren(h('div', { class: 'network-node-title', text: t('network.hub.merge') }), split);
        put(el, at.merge, at.merge.size);
        seen.add('merge');
      }
      for (const l of m.links) {
        const id = 'link:' + l.name;
        const el = node(id, 'link', (l.primary ? 'primary ' : '') + (l.away ? 'away ' : '') + (at[id].inside ? 'inside' : ''));
        el.title = l.primary ? t('network.link.primary') : l.away ? t('network.link.away') : '';
        el.firstChild.replaceChildren(
          h('div', { class: 'network-node-title', text: l.label || l.name }),
          h('div', { class: 'network-node-sub', text: l.away ? t('network.link.away') : l.name }),
          h('div', { class: 'network-node-speed', text: l.away ? '' : speed(l.down) }));
        put(el, at[id]);
        seen.add(id);
      }
      for (const a of m.apps) {
        const id = 'app:' + a.name;
        const el = node(id, 'app', (a.wire ? 'wired ' : '') + (a.idle ? 'idle ' : '') + (picked === a.name ? 'picked' : ''));
        el.title = a.idle ? t('network.app.idle') : '';
        el.firstChild.replaceChildren(
          h('div', { class: 'network-node-title', text: a.name }),
          h('div', { class: 'network-node-sub', text: speed(a.down) + ' / ' + speed(a.up) + (a.wire ? '  ' + t('network.priority.' + a.wire.priority) : '') }));
        put(el, at[id]);
        seen.add(id);
      }
      for (const [id, el] of els) if (!seen.has(id)) { el.remove(); els.delete(id); }
      let w = 0, hgt = 0;
      for (const [id, p] of Object.entries(at)) {
        w = Math.max(w, p.x + (p.size || (id.startsWith('app:') ? APP_W : D)) + 400);
        hgt = Math.max(hgt, p.y + (p.size || D) + 300);
      }
      board.style.width = Math.max(w, scroller.clientWidth || 900) + 'px';
      board.style.height = Math.max(hgt, scroller.clientHeight || 600) + 'px';
      wires();
      if (picked) fillCard(picked);
    }

    // Anchors: the right edge of a circle, the left edge of an app box.
    const out = (id) => {
      const { at } = current;
      const p = at[id];
      if (!p) return null;
      if (id === 'merge') return { x: p.x + p.size, y: p.y + p.size / 2 };
      return { x: p.x + D, y: p.y + D / 2 };
    };
    const into = (id) => { const p = current.at[id]; return p && { x: p.x, y: p.y + APP_H / 2 }; };
    const thick = (b) => 1.5 + Math.min(6, Math.log10(1 + (Number(b) || 0) / 1e4) * 1.6);
    const line = (from, to, cls, width, curved) => {
      const p = document.createElementNS(NS, 'path');
      const mx = (from.x + to.x) / 2;
      p.setAttribute('d', curved ? `M ${from.x} ${from.y} C ${mx} ${from.y}, ${mx} ${to.y}, ${to.x} ${to.y}` : `M ${from.x} ${from.y} L ${to.x} ${to.y}`);
      p.setAttribute('class', cls);
      p.style.strokeWidth = width + 'px';
      svg.append(p);
      return p;
    };
    function wires() {
      svg.replaceChildren();
      const { m, at } = current;
      const control = N().control && N().running;
      // The straight wire between the connections of the Combined circle.
      for (let i = 1; i < m.merge.length; i++) {
        const a = at['link:' + m.merge[i - 1]], b = at['link:' + m.merge[i]];
        line({ x: a.x + D, y: a.y + D / 2 }, { x: b.x, y: b.y + D / 2 }, 'network-wire join', 3);
      }
      for (const a of m.apps) {
        const to = into('app:' + a.name);
        if (!to) continue;
        if (a.wire && a.wire.target !== 'system') {
          const from = out(a.wire.target === 'merge' ? (m.merge.length ? 'merge' : null) : a.wire.target);
          if (from) line(from, to, 'network-wire ' + (control ? a.wire.priority : 'planned'), control ? thick(a.down) : 1.8, true);
          continue;
        }
        if (a.idle) continue;
        // Wired by itself: the connections its sockets were seen on, else the one macOS uses.
        const seen = (a.interfaces || []).map((x) => 'link:' + x.name).filter((id) => at[id]);
        const primary = m.links.find((l) => l.primary);
        const froms = seen.length ? seen : primary ? ['link:' + primary.name] : [];
        for (const id of froms) line(out(id), to, 'network-wire seen', thick(a.down), true);
      }
    }

    // Moving a node: it follows the pointer, its wires with it, and it lands on the grid.
    function startMove(e, id) {
      if (e.button !== 0) return;
      e.preventDefault();
      // A connection inside the Combined circle moves the circle.
      if (current.at[id] && current.at[id].inside) id = 'merge';
      const el = els.get(id);
      const start = { x: e.clientX, y: e.clientY };
      const from = { x: current.at[id].x, y: current.at[id].y };
      let moved = false;
      moving = id;
      el.setPointerCapture(e.pointerId);
      const move = (ev) => {
        const dx = ev.clientX - start.x, dy = ev.clientY - start.y;
        if (!moved && Math.abs(dx) + Math.abs(dy) < 4) return;
        moved = true;
        const p = { x: Math.max(0, from.x + dx), y: Math.max(0, from.y + dy) };
        current.at[id].x = p.x; current.at[id].y = p.y;
        if (id === 'merge') {
          current.m.merge.forEach((name, i) => {
            const c = current.at['link:' + name];
            const size = current.at.merge.size;
            c.x = p.x + size / 2 + (i - (current.m.merge.length - 1) / 2) * CHILD - D / 2;
            c.y = p.y + size / 2 - D / 2;
            put(els.get('link:' + name), c);
          });
        }
        put(el, p);
        wires();
      };
      const up = () => {
        el.removeEventListener('pointermove', move);
        el.removeEventListener('pointerup', up);
        el.removeEventListener('pointercancel', up);
        moving = null;
        if (moved) {
          const p = { x: snap(current.at[id].x), y: snap(current.at[id].y) };
          local.set(id, p);
          send1('network.place', { id, x: p.x, y: p.y });
        } else if (id.startsWith('app:')) {
          pick(id.slice(4));
        }
        sync();
      };
      el.addEventListener('pointermove', move);
      el.addEventListener('pointerup', up);
      el.addEventListener('pointercancel', up);
    }

    // Wiring: from the dot of a node to another node.
    function startWire(e, id) {
      if (e.button !== 0) return;
      e.preventDefault();
      e.stopPropagation();
      const box = () => board.getBoundingClientRect();
      const from = id.startsWith('app:') ? into(id) : out(id);
      wiring = { id, line: null };
      const target = (ev) => {
        const over = document.elementFromPoint(ev.clientX, ev.clientY);
        const el = over && over.closest && over.closest('[data-node]');
        return el && el.dataset.node !== id ? el : null;
      };
      const move = (ev) => {
        if (wiring.line) wiring.line.remove();
        const b = box();
        wiring.line = line(from, { x: ev.clientX - b.left, y: ev.clientY - b.top }, 'network-wire dragging', 2.5, false);
        const over = target(ev);
        for (const el of els.values()) el.classList.toggle('drop', el === over);
      };
      const up = (ev) => {
        window.removeEventListener('pointermove', move);
        window.removeEventListener('pointerup', up);
        const over = target(ev);
        wiring = null;
        for (const el of els.values()) el.classList.remove('drop');
        if (over) connect(id, over.dataset.node);
        sync();
      };
      window.addEventListener('pointermove', move);
      window.addEventListener('pointerup', up);
    }

    // What a wire from one node to another means.
    function connect(a, b) {
      const kind = (id) => id === 'merge' ? 'merge' : id.split(':')[0];
      const link = (id) => id.slice(5);
      const merge = (N().merge || []).slice();
      const place = (id) => id === 'merge' ? 'merge' : 'link:' + link(id);
      if (kind(a) === 'app' && kind(b) !== 'app') return send1('network.wire', { app: a.slice(4), target: place(b) });
      if (kind(b) === 'app' && kind(a) !== 'app') return send1('network.wire', { app: b.slice(4), target: place(a) });
      if (kind(a) === 'app') return;
      // A connection wired to another, or to the circle: the Combined circle around them.
      const names = [a, b].filter((id) => id !== 'merge').map(link);
      const joined = merge.length && (names.some((x) => merge.includes(x)) || a === 'merge' || b === 'merge') ? merge : [];
      for (const x of names) if (!joined.includes(x)) joined.push(x);
      if (joined.length >= 2) send1('network.merge', joined.slice(0, 4));
    }

    // ---- the card of one app
    const cardTitle = h('h3');
    const held = h('p', { class: 'network-note' });
    const targetSel = h('select', { class: 'select', 'aria-label': t('network.card.target') });
    const prioSel = h('select', { class: 'select', 'aria-label': t('network.card.priority') });
    for (const p of ['high', 'normal', 'low']) prioSel.append(h('option', { value: p, text: t('network.priority.' + p) }));
    const downIn = h('input', { type: 'number', min: '0', max: '1000000', step: '64', class: 'text-field network-limit', placeholder: t('network.card.limit.none'), 'aria-label': t('network.card.down') });
    const upIn = h('input', { type: 'number', min: '0', max: '1000000', step: '64', class: 'text-field network-limit', placeholder: t('network.card.limit.none'), 'aria-label': t('network.card.up') });
    const field = (label, control) => h('label', { class: 'network-field' }, h('span', { text: label }), control);
    const limit = (input) => { const v = Math.round(Number(input.value)); return v > 0 ? Math.min(v, 1000000) : 0; };
    const save = button(t('network.card.save'), () => {
      if (!picked) return;
      send1('network.rule', { app: picked, target: targetSel.value, priority: prioSel.value, down_kbps: limit(downIn), up_kbps: limit(upIn) });
    }, { primary: true, small: true });
    const unwire = button(t('network.card.unwire'), () => { if (picked) send1('network.unwire', picked); }, { small: true });
    const shut = button(t('network.close'), () => unpick(), { small: true });
    detail.append(cardTitle, held,
      h('div', { class: 'network-fields' },
        field(t('network.card.target'), targetSel), field(t('network.card.priority'), prioSel),
        field(t('network.card.down'), downIn), field(t('network.card.up'), upIn)),
      h('div', { class: 'network-actions' }, shut, unwire, save));
    function pick(app) {
      picked = app;
      detail.hidden = false;
      fillCard(app, true);
      for (const [id, el] of els) el.classList.toggle('picked', id === 'app:' + app);
    }
    function unpick() {
      picked = null;
      detail.hidden = true;
      for (const el of els.values()) el.classList.remove('picked');
    }
    function fillCard(app, fresh) {
      const n = N();
      const wire = (n.wires || []).find((w) => w.app.toLowerCase() === app.toLowerCase());
      const a = (n.apps || []).find((x) => x.name === app);
      cardTitle.textContent = app;
      held.textContent = a && a.held_to ? t('network.card.held', { speed: speed(a.held_to) }) : '';
      held.hidden = !held.textContent;
      if (!fresh) return;
      const links = current ? current.m.links : [];
      const options = [['system', t('network.hub.system')], ['merge', (n.merge || []).length >= 2 ? t('network.hub.merge') + ' (' + n.merge.map(linkName).join(' + ') + ')' : t('network.hub.merge.all')],
        ...links.map((l) => ['link:' + l.name, (l.label || l.name) + ' (' + l.name + ')'])];
      targetSel.replaceChildren(...options.map(([v, label]) => h('option', { value: v, text: label })));
      targetSel.value = wire ? wire.target : 'system';
      prioSel.value = wire ? wire.priority : 'normal';
      downIn.value = wire && wire.down_kbps ? wire.down_kbps : '';
      upIn.value = wire && wire.up_kbps ? wire.up_kbps : '';
      unwire.hidden = !wire;
    }

    // Only the canvas window listens to the whole window; the self-test draws the canvas inside the settings page.
    if (window.__canvas) {
      window.addEventListener('keydown', (e) => {
        if (e.key !== 'Escape') return;
        if (!detail.hidden) unpick(); else send({ t: 'action', name: 'network_canvas_close' });
      });
      window.addEventListener('resize', () => sync());
    }
    reg(sync);
    return root;
  },
  selftest({ expect, go, sent, S }) {
    go('network');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const was = S.network.enabled;
    page.querySelector('.switch[data-path="network.enabled"]').click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'network.enabled' && m.value === !was), 'network: the switch sends its setting: ' + JSON.stringify(sent));
    page.querySelector('.switch[data-path="network.enabled"]').click();
    expect(page.querySelector('.network-table').textContent.includes('Steam'), 'network: the apps are listed with what they move');
    sent.length = 0;
    [...page.querySelectorAll('.btn')].find((b) => b.textContent === t('network.open')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'network_canvas_open'), 'network: the diagram opens in a window of its own');

    // The canvas, drawn here as its window draws it.
    const holder = document.createElement('div');
    holder.style.cssText = 'position:fixed;left:0;top:0;width:1100px;height:700px;';
    document.body.append(holder);
    let canvas;
    const syncsBefore = syncs.length;
    canvas = this.canvas();
    holder.append(canvas);
    runSyncs();
    const nodes = (kind) => [...canvas.querySelectorAll('.network-node.' + kind)];
    expect(canvas.querySelector('.network-node.merge'), 'network: two connections wired together are the Combined circle');
    expect(nodes('link').filter((n) => n.classList.contains('inside')).length === 2, 'network: the circle holds the two connections');
    const names = nodes('app').map((n) => n.dataset.node.slice(4));
    expect(names.includes('Spotify'), 'network: an app seen before stays on the canvas while it is idle: ' + names);
    const steam = nodes('app').find((n) => n.dataset.node === 'app:Steam');
    expect(steam && steam.style.left === '560px' && steam.style.top === '80px', 'network: a box stays where it was put');
    expect(canvas.querySelectorAll('.network-wire.join').length === 1, 'network: the straight wire joins the two connections');
    expect(canvas.querySelectorAll('.network-wire').length >= 4, 'network: the apps are wired');
    sent.length = 0;
    canvas.querySelector('.network-node.merge .btn').click();
    expect(sent.some((m) => m.path === 'network.merge' && Array.isArray(m.value) && m.value.length === 0), 'network: the circle can be taken apart');
    sent.length = 0;
    [...canvas.querySelectorAll('.btn')].find((b) => b.textContent === t('network.close')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'network_canvas_close'), 'network: the window has its close button');
    holder.remove();
    syncs.splice(syncsBefore);

    S.network.tether.running = true; runSyncs();
    const stop = [...page.querySelectorAll('.btn')].find((b) => b.textContent === t('network.tether.stop'));
    expect(stop && !stop.hidden, 'network: sharing with the phone can be stopped');
    sent.length = 0;
    stop.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'network_tether_stop'), 'network: stop sends its action');
    S.network.tether.running = false; S.network.status = 'denied'; runSyncs();
    expect(page.textContent.includes(t('network.status.denied')), 'network: a refused password says what to do');
    S.network.status = 'on'; runSyncs();
  },
});

// Formatting shared by the page and the canvas.
function netHelpers() {
  const speed = (b) => {
    const n = Number(b) || 0;
    if (n >= 1e9) return (n / 1e9).toFixed(1) + ' GB/s';
    if (n >= 1e6) return (n / 1e6).toFixed(1) + ' MB/s';
    if (n >= 1e3) return Math.round(n / 1e3) + ' KB/s';
    return n + ' B/s';
  };
  const size = (b) => {
    const n = Number(b) || 0;
    if (n >= 1e9) return (n / 1e9).toFixed(2) + ' GB';
    if (n >= 1e6) return (n / 1e6).toFixed(1) + ' MB';
    if (n >= 1e3) return Math.round(n / 1e3) + ' KB';
    return n + ' B';
  };
  const linkName = (iface) => {
    const all = [...(S.network.links || []), ...(S.network.known_links || [])];
    const l = all.find((x) => x.name === iface);
    return l ? l.label : iface;
  };
  return { speed, size, linkName };
}
