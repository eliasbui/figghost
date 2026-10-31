// Network: a diagram of the Mac's connections wired to the apps that use them, what each app moves, and (beta) wires
// that choose the connection of an app, its speed and its place behind the apps wired before it.
(window.__plugins = window.__plugins || []).push({
  id: 'network',
  icon: 'network',
  en: {
    'nav.network': 'Network', 'page.network': 'Network',
    'page.network.sub': 'See which app uses which connection and how much, and wire an app to Wi-Fi, Ethernet or both.',
    'network.enable.title': 'Watch the network', 'network.enable.desc': 'Reads what each app sends and receives, once a second and only while this page is open. Nothing leaves the Mac.',
    'network.open': 'Open the diagram', 'network.open.desc': 'The connections on the left, where they meet in the middle, the apps on the right. Drag from an app to a place to wire it.',
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
    'network.watching': 'Watching only: the wires show where each app goes. Turn on Wiring to choose.',
    'network.drag': 'Drag from the dot of an app to a connection or to Combined. The first app wired to a place is the main one; apps wired after it come below it when the connection is full.',
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
    'network.open': 'Mở sơ đồ', 'network.open.desc': 'Bên trái là các đường mạng, ở giữa là chỗ chúng gặp nhau, bên phải là các app. Kéo từ một app sang một chỗ để nối dây.',
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
    'network.watching': 'Chỉ đang theo dõi: dây cho thấy mỗi app đang đi đường nào. Bật Nối dây để tự chọn.',
    'network.drag': 'Kéo từ chấm của một app sang một đường mạng hoặc sang Gộp. App nối vào một chỗ đầu tiên là app chính; app nối sau sẽ xếp dưới khi đường đó đầy.',
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
      const l = (N().links || []).find((x) => x.name === iface);
      return l ? l.label : iface;
    };
    const wireOf = (app) => (N().wires || []).find((w) => w.app.toLowerCase() === app.toLowerCase());
    const targetLabel = (target) => target === 'merge' ? t('network.hub.merge') : target === 'system' ? t('network.hub.system') : linkName(String(target).replace('link:', ''));

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

    // ---- the diagram, in a sheet over the whole window
    const board = h('div', { class: 'network-board' });
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('class', 'network-wires');
    const nodes = h('div', { class: 'network-nodes' });
    board.append(svg, nodes);
    const note = h('p', { class: 'network-note' });
    const detail = h('div', { class: 'network-card', hidden: true });
    const closeBtn = button(t('network.close'), () => closeBoard(), { small: true });
    const scrim = h('div', { class: 'network-scrim', hidden: true },
      h('div', { class: 'network-sheet', role: 'dialog', 'aria-modal': 'true', 'aria-label': t('page.network') },
        h('div', { class: 'network-sheet-head' },
          h('h2', { text: t('page.network') }), h('span', { class: 'network-total-head' }), closeBtn),
        note, board, detail));
    let picked = null;
    let dragging = null;
    const key = (e) => { if (e.key === 'Escape') { e.stopPropagation(); if (!detail.hidden) { detail.hidden = true; picked = null; } else closeBoard(); } };
    function openBoard() {
      scrim.hidden = false;
      window.addEventListener('keydown', key, true);
      draw();
      closeBtn.focus();
    }
    function closeBoard() {
      scrim.hidden = true;
      detail.hidden = true;
      picked = null;
      window.removeEventListener('keydown', key, true);
    }
    scrim.addEventListener('click', (e) => { if (e.target === scrim) closeBoard(); });

    const NS = 'http://www.w3.org/2000/svg';
    const path = (from, to, cls, width) => {
      const p = document.createElementNS(NS, 'path');
      const mx = (from.x + to.x) / 2;
      p.setAttribute('d', `M ${from.x} ${from.y} C ${mx} ${from.y}, ${mx} ${to.y}, ${to.x} ${to.y}`);
      p.setAttribute('class', cls);
      p.style.strokeWidth = width + 'px';
      svg.append(p);
      return p;
    };
    // Thicker for more traffic: 1.5 points idle, about 7 at 100 MB/s.
    const thick = (b) => 1.5 + Math.min(6, Math.log10(1 + (Number(b) || 0) / 1e4) * 1.6);

    function draw() {
      if (scrim.hidden || dragging) return;
      const n = N();
      const control = n.control && n.running;
      note.textContent = n.paused ? t('network.paused') : control ? t('network.drag') : t('network.watching');
      scrim.querySelector('.network-total-head').textContent = t('network.total') + ': ' + speed(n.total_down) + ' / ' + speed(n.total_up);
      nodes.replaceChildren();
      svg.replaceChildren();
      const width = board.clientWidth || 760;
      const cols = { links: 16, meet: Math.round(width * 0.36), apps: Math.round(width - 236) };
      const node = (cls, x, y, target, kids) => {
        const el = h('div', { class: 'network-node ' + cls, style: `left:${x}px; top:${y}px`, 'data-target': target || null }, kids);
        nodes.append(el);
        return el;
      };
      const header = (x, text) => nodes.append(h('div', { class: 'network-col', style: `left:${x}px`, text }));
      header(cols.links, t('network.links'));
      header(cols.meet, t('network.meet'));
      header(cols.apps, t('network.apps'));
      const links = n.links || [];
      const anchor = {};
      links.forEach((l, i) => {
        const y = 36 + i * 74;
        const el = node('link' + (l.primary ? ' primary' : ''), cols.links, y, 'link:' + l.name, [
          h('div', { class: 'network-node-title', text: l.label }),
          h('div', { class: 'network-node-sub', text: l.name + '  ' + l.ip }),
          h('div', { class: 'network-node-speed', text: speed(l.down) + ' / ' + speed(l.up) }),
        ]);
        if (l.primary) el.title = t('network.link.primary');
        anchor['link:' + l.name] = { x: cols.links + 190, y: y + 30, el };
      });
      if (!links.length) node('empty', cols.links, 36, null, [h('div', { class: 'network-node-sub', text: t('network.links.none') })]);
      const hubs = [['merge', 36], ['system', 136]];
      for (const [id, y] of hubs) {
        const el = node('hub ' + id, cols.meet, y, id, [
          h('div', { class: 'network-node-title', text: t('network.hub.' + id) }),
          h('div', { class: 'network-node-sub', text: t('network.hub.' + id + '.desc') }),
        ]);
        anchor[id] = { x: cols.meet, y: y + 30, out: { x: cols.meet + 190, y: y + 30 }, el };
      }
      // Every link that has an address feeds Combined; the one macOS uses feeds Automatic.
      for (const l of links) {
        const a = anchor['link:' + l.name];
        path(a, anchor.merge, 'network-wire feed', 1.5);
        if (l.primary) path(a, anchor.system, 'network-wire feed', 1.5);
      }
      // The apps: those wired, in the order of their place and priority, then the others by what they move.
      const rank = { high: 0, normal: 1, low: 2 };
      const apps = (n.apps || []).slice();
      for (const w of n.wires || []) if (!apps.some((a) => a.name.toLowerCase() === w.app.toLowerCase())) apps.push({ name: w.app, down: 0, up: 0, received: 0, sent: 0, interfaces: [] });
      apps.sort((a, b) => {
        const wa = wireOf(a.name), wb = wireOf(b.name);
        if (!!wa !== !!wb) return wa ? -1 : 1;
        if (wa && wb && wa.target !== wb.target) return wa.target < wb.target ? -1 : 1;
        if (wa && wb && rank[wa.priority] !== rank[wb.priority]) return rank[wa.priority] - rank[wb.priority];
        return (b.down || 0) - (a.down || 0);
      });
      const shown = apps.slice(0, 18);
      board.style.height = Math.max(240, 36 + Math.max(links.length * 74, 220, shown.length * 46) + 16) + 'px';
      shown.forEach((a, i) => {
        const y = 36 + i * 46;
        const wire = wireOf(a.name);
        const port = h('span', { class: 'network-port', 'aria-hidden': 'true' });
        const el = node('app' + (wire ? ' wired' : '') + (picked === a.name ? ' picked' : ''), cols.apps, y, null, [
          port,
          h('div', { class: 'network-node-title', text: a.name }),
          h('div', { class: 'network-node-sub', text: speed(a.down) + ' / ' + speed(a.up) + (wire ? '  ' + t('network.priority.' + wire.priority) : '') }),
        ]);
        el.dataset.app = a.name;
        el.tabIndex = 0;
        el.addEventListener('click', () => pick(a.name));
        el.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(a.name); } });
        port.addEventListener('pointerdown', (e) => startDrag(e, a.name, { x: cols.apps, y: y + 20 }));
        const at = { x: cols.apps, y: y + 20 };
        const cls = 'network-wire' + ((a.down || 0) > 0 ? ' live' : '');
        if (wire && control) {
          const to = wire.target.startsWith('link:') ? anchor[wire.target] : anchor[wire.target] && anchor[wire.target].out;
          if (to) path(to, at, cls + ' ' + wire.priority, thick(a.down));
        } else if (wire) {
          const to = wire.target.startsWith('link:') ? anchor[wire.target] : anchor[wire.target] && anchor[wire.target].out;
          if (to) path(to, at, 'network-wire planned', 1.5);
        } else {
          // Not wired: where it goes now, from what was seen of its sockets.
          const seen = (a.interfaces || []).filter((x) => anchor['link:' + x.name]);
          if (seen.length) for (const x of seen) path(anchor['link:' + x.name], at, cls + ' seen', thick(x.down));
          else path(anchor.system.out, at, 'network-wire seen', 1.5);
        }
      });
      svg.setAttribute('width', width);
      svg.setAttribute('height', board.style.height);
      if (picked) fillCard(picked);
    }
    reg(draw);
    window.addEventListener('resize', () => draw());

    function startDrag(e, app, from) {
      if (!N().control) return;
      e.preventDefault();
      e.stopPropagation();
      const box = board.getBoundingClientRect();
      dragging = { app, line: null };
      const move = (ev) => {
        if (dragging.line) dragging.line.remove();
        dragging.line = path({ x: ev.clientX - box.left, y: ev.clientY - box.top }, from, 'network-wire dragging', 2.5);
        for (const el of nodes.querySelectorAll('[data-target]')) el.classList.toggle('drop', el.contains(document.elementFromPoint(ev.clientX, ev.clientY)));
      };
      const up = (ev) => {
        window.removeEventListener('pointermove', move);
        window.removeEventListener('pointerup', up);
        const over = document.elementFromPoint(ev.clientX, ev.clientY);
        const target = over && over.closest && over.closest('[data-target]');
        dragging = null;
        if (target) send({ t: 'set', path: 'network.wire', value: { app, target: target.dataset.target } });
        draw();
      };
      window.addEventListener('pointermove', move);
      window.addEventListener('pointerup', up);
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
      send({ t: 'set', path: 'network.rule', value: { app: picked, target: targetSel.value, priority: prioSel.value, down_kbps: limit(downIn), up_kbps: limit(upIn) } });
    }, { primary: true, small: true });
    const unwire = button(t('network.card.unwire'), () => { if (picked) send({ t: 'set', path: 'network.unwire', value: picked }); }, { small: true });
    detail.append(cardTitle, held,
      h('div', { class: 'network-fields' },
        field(t('network.card.target'), targetSel), field(t('network.card.priority'), prioSel),
        field(t('network.card.down'), downIn), field(t('network.card.up'), upIn)),
      h('div', { class: 'network-actions' }, unwire, save));
    function pick(app) {
      picked = app;
      detail.hidden = false;
      fillCard(app, true);
      for (const el of nodes.querySelectorAll('.network-node.app')) el.classList.toggle('picked', el.dataset.app === app);
    }
    function fillCard(app, fresh) {
      const n = N();
      const wire = wireOf(app);
      const a = (n.apps || []).find((x) => x.name === app);
      cardTitle.textContent = app;
      held.textContent = a && a.held_to ? t('network.card.held', { speed: speed(a.held_to) }) : '';
      held.hidden = !held.textContent;
      const options = [['system', t('network.hub.system')], ['merge', t('network.hub.merge')], ...(n.links || []).map((l) => ['link:' + l.name, l.label + ' (' + l.name + ')'])];
      if (wire && !options.some(([v]) => v === wire.target)) options.push([wire.target, targetLabel(wire.target)]);
      // Only a fresh card is filled: a card being edited keeps what the person chose while the speeds change.
      if (!fresh) return;
      targetSel.replaceChildren(...options.map(([v, label]) => h('option', { value: v, text: label })));
      targetSel.value = wire ? wire.target : 'system';
      prioSel.value = wire ? wire.priority : 'normal';
      downIn.value = wire && wire.down_kbps ? wire.down_kbps : '';
      upIn.value = wire && wire.up_kbps ? wire.up_kbps : '';
      for (const el of [targetSel, prioSel, downIn, upIn, save]) el.disabled = !n.control;
      unwire.hidden = !wire;
    }

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
        row(t('network.open'), t('network.open.desc'), button(t('network.open'), openBoard, { primary: true, small: true }), when)),
      sectionTitle(t('sec.network.summary')),
      card(row(total, null, null, when), h('div', { class: 'row block network-table-row' }, table)),
      sectionTitle(t('sec.network.control')),
      card(toggleRow('network.control', t('network.control.title'), t('network.control.desc'), when),
        row(status, null, null, when), restoreRow),
      sectionTitle(t('sec.network.tether')),
      card(row(t('network.tether.title'), t('network.tether.desc'), h('div', { class: 'network-phone' }, phone, start, stop), when),
        h('p', { class: 'network-note', text: t('network.tether.iphone') })),
      scrim,
    ];
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

    const open = [...page.querySelectorAll('.btn')].find((b) => b.textContent === t('network.open'));
    open.click();
    const scrim = document.querySelector('.network-scrim');
    expect(!scrim.hidden, 'network: the diagram opens');
    const links = scrim.querySelectorAll('.network-node.link');
    expect(links.length === S.network.links.length, 'network: one node for each connection');
    expect(scrim.querySelector('.network-node.hub.merge') && scrim.querySelector('.network-node.hub.system'), 'network: the two places where connections meet');
    expect(scrim.querySelectorAll('.network-wire').length >= S.network.links.length + 3, 'network: the wires are drawn');
    const steam = [...scrim.querySelectorAll('.network-node.app')].find((n) => n.dataset.app === 'Steam');
    expect(steam && steam.classList.contains('wired'), 'network: a wired app says so');
    steam.click();
    const card = scrim.querySelector('.network-card');
    expect(!card.hidden && card.querySelector('h3').textContent === 'Steam', 'network: a click opens the card of the app');
    expect(card.querySelector('select').value === 'link:en7', 'network: the card shows the wire of the app');
    sent.length = 0;
    card.querySelector('.network-limit').value = '1024';
    [...card.querySelectorAll('.btn')].find((b) => b.textContent === t('network.card.save')).click();
    expect(sent.some((m) => m.path === 'network.rule' && m.value.app === 'Steam' && m.value.down_kbps === 1024 && m.value.target === 'link:en7'), 'network: the card sends the whole rule: ' + JSON.stringify(sent));
    sent.length = 0;
    [...card.querySelectorAll('.btn')].find((b) => b.textContent === t('network.card.unwire')).click();
    expect(sent.some((m) => m.path === 'network.unwire' && m.value === 'Steam'), 'network: the wire can be removed');
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    expect(scrim.hidden, 'network: Escape closes the card, then the diagram');

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
