// Mole (github.com/tw93/Mole): system health, cleaning and optimizing through the user's own `mo` program.
// The state is under S.mole; every action is checked again by the app.
(function () {
  const en = {
    'nav.mole': 'Mole',
    'page.mole': 'Mole',
    'page.mole.sub': 'Check the health of this Mac and free disk space, with the Mole command line tool.',
    'mole.missing.title': 'Mole is not installed',
    'mole.missing.desc': 'Install it with Homebrew: brew install mole. Figghost only runs the mo program; nothing is bundled.',
    'mole.site': 'Open Mole',
    'mole.sec.health': 'Health',
    'mole.health.title': 'Health score',
    'mole.health.none': 'Not read yet',
    'mole.health.refresh': 'Refresh',
    'mole.cpu': 'CPU', 'mole.memory': 'Memory', 'mole.disk': 'Disk',
    'mole.swap': 'Swap {used} of {total}',
    'mole.sec.clean': 'Clean',
    'mole.clean.title': 'Free up disk space',
    'mole.clean.desc': 'Caches, logs, temporary files and leftovers of apps that are gone. Preview first: nothing is deleted until you choose Clean now. Items that need an administrator password are skipped.',
    'mole.clean.confirm.title': 'Clean now?',
    'mole.clean.confirm.body': 'Mole deletes the items it found. Run a preview first if you have not seen the list.',
    'mole.clean.run': 'Clean now',
    'mole.sec.optimize': 'Optimize',
    'mole.optimize.title': 'Refresh caches and services',
    'mole.optimize.desc': 'Rebuilds Finder and system caches, refreshes the network stack and repairs broken settings.',
    'mole.optimize.confirm.title': 'Optimize now?',
    'mole.optimize.confirm.body': 'Mole refreshes caches and services. Run a preview first if you want to see what changes.',
    'mole.optimize.run': 'Optimize now',
    'mole.preview': 'Preview',
    'mole.working': 'Working, this can take a minute',
    'mole.sec.report': 'Result',
    'mole.report.preview': 'Preview of {task}',
    'mole.report.done': 'Result of {task}',
    'mole.report.failed': 'Did not finish',
    'mole.task.clean': 'clean', 'mole.task.optimize': 'optimize',
    'mole.report.empty': 'Run a preview to see what Mole would do.',
    'mole.sec.purge': 'Project leftovers',
    'mole.purge.title': 'Remove old build folders',
    'mole.purge.desc': 'node_modules, build and virtual environment folders of projects you have not touched for a while, found in the usual project folders.',
    'mole.purge.confirm.title': 'Remove these folders?',
    'mole.purge.confirm.body': 'Mole deletes the build folders it found. They can be rebuilt, but they are not sent to the Trash.',
    'mole.purge.run': 'Remove now',
    'mole.sec.installer': 'Installers',
    'mole.installer.title': 'Old installer files',
    'mole.installer.desc': 'Finds .dmg, .pkg, .iso, .xip and .zip files. Mole asks you to pick them in a list, so this opens it in Terminal.',
    'mole.installer.open': 'Open in Terminal',
    'mole.sec.uninstall': 'Uninstall apps',
    'mole.uninstall.title': 'Remove an app with its leftovers',
    'mole.uninstall.desc': 'Apps and what they left in Library go to the Trash, so they can be put back.',
    'mole.uninstall.scan': 'List apps',
    'mole.uninstall.empty': 'List the apps to choose one.',
    'mole.uninstall.run': 'Uninstall',
    'mole.uninstall.confirm.title': 'Uninstall {name}?',
    'mole.uninstall.confirm.body': 'The app and its leftover files go to the Trash.',
    'mole.sec.analyze': 'Disk usage',
    'mole.analyze.title': 'What takes the space',
    'mole.analyze.desc': 'The biggest folders first. Choose a folder to look inside it.',
    'mole.analyze.scan': 'Scan this Mac',
    'mole.analyze.up': 'Up',
    'mole.analyze.empty': 'Scan to see where the space went.',
    'mole.sec.history': 'History',
    'mole.history.title': 'Recent runs',
    'mole.history.desc': 'What Mole did lately, from its own log.',
    'mole.history.scan': 'Show history',
    'mole.history.empty': 'Nothing to show yet.',
    'mole.history.items': '{n} items',
    'mole.task.purge': 'purge', 'mole.task.uninstall': 'uninstall',
    'fault.mole_missing': 'Mole is not installed.',
    'fault.mole_terminal': 'Could not open Terminal.',
  };
  const vi = {
    'nav.mole': 'Mole',
    'page.mole': 'Mole',
    'page.mole.sub': 'Kiểm tra sức khỏe máy Mac và giải phóng dung lượng bằng công cụ dòng lệnh Mole.',
    'mole.missing.title': 'Chưa cài Mole',
    'mole.missing.desc': 'Cài bằng Homebrew: brew install mole. Figghost chỉ chạy chương trình mo, không đóng gói kèm.',
    'mole.site': 'Mở Mole',
    'mole.sec.health': 'Sức khỏe',
    'mole.health.title': 'Điểm sức khỏe',
    'mole.health.none': 'Chưa đọc',
    'mole.health.refresh': 'Đọc lại',
    'mole.cpu': 'CPU', 'mole.memory': 'Bộ nhớ', 'mole.disk': 'Ổ đĩa',
    'mole.swap': 'Swap {used} trên {total}',
    'mole.sec.clean': 'Dọn dẹp',
    'mole.clean.title': 'Giải phóng dung lượng',
    'mole.clean.desc': 'Bộ nhớ đệm, nhật ký, tệp tạm và phần còn sót của ứng dụng đã xóa. Hãy xem trước: không có gì bị xóa cho đến khi bạn chọn Dọn ngay. Mục cần mật khẩu quản trị sẽ bị bỏ qua.',
    'mole.clean.confirm.title': 'Dọn ngay?',
    'mole.clean.confirm.body': 'Mole sẽ xóa các mục đã tìm thấy. Hãy chạy xem trước nếu bạn chưa xem danh sách.',
    'mole.clean.run': 'Dọn ngay',
    'mole.sec.optimize': 'Tối ưu',
    'mole.optimize.title': 'Làm mới bộ nhớ đệm và dịch vụ',
    'mole.optimize.desc': 'Dựng lại bộ nhớ đệm của Finder và hệ thống, làm mới mạng và sửa các thiết lập hỏng.',
    'mole.optimize.confirm.title': 'Tối ưu ngay?',
    'mole.optimize.confirm.body': 'Mole sẽ làm mới bộ nhớ đệm và dịch vụ. Hãy chạy xem trước nếu muốn biết điều gì thay đổi.',
    'mole.optimize.run': 'Tối ưu ngay',
    'mole.preview': 'Xem trước',
    'mole.working': 'Đang chạy, có thể mất khoảng một phút',
    'mole.sec.report': 'Kết quả',
    'mole.report.preview': 'Xem trước {task}',
    'mole.report.done': 'Kết quả {task}',
    'mole.report.failed': 'Chưa hoàn tất',
    'mole.task.clean': 'dọn dẹp', 'mole.task.optimize': 'tối ưu',
    'mole.report.empty': 'Hãy chạy xem trước để biết Mole sẽ làm gì.',
    'mole.sec.purge': 'Phần còn sót của dự án',
    'mole.purge.title': 'Xóa thư mục build cũ',
    'mole.purge.desc': 'node_modules, thư mục build và môi trường ảo của các dự án lâu chưa đụng tới, tìm trong các thư mục dự án thông dụng.',
    'mole.purge.confirm.title': 'Xóa các thư mục này?',
    'mole.purge.confirm.body': 'Mole xóa các thư mục build đã tìm thấy. Chúng dựng lại được nhưng không được chuyển vào Thùng rác.',
    'mole.purge.run': 'Xóa ngay',
    'mole.sec.installer': 'Bộ cài',
    'mole.installer.title': 'Tệp cài đặt cũ',
    'mole.installer.desc': 'Tìm các tệp .dmg, .pkg, .iso, .xip và .zip. Mole để bạn chọn trong một danh sách nên mục này mở nó trong Terminal.',
    'mole.installer.open': 'Mở trong Terminal',
    'mole.sec.uninstall': 'Gỡ ứng dụng',
    'mole.uninstall.title': 'Gỡ ứng dụng cùng phần còn sót',
    'mole.uninstall.desc': 'Ứng dụng và những gì nó để lại trong Library được chuyển vào Thùng rác nên có thể khôi phục.',
    'mole.uninstall.scan': 'Liệt kê ứng dụng',
    'mole.uninstall.empty': 'Hãy liệt kê ứng dụng để chọn.',
    'mole.uninstall.run': 'Gỡ',
    'mole.uninstall.confirm.title': 'Gỡ {name}?',
    'mole.uninstall.confirm.body': 'Ứng dụng và các tệp còn sót được chuyển vào Thùng rác.',
    'mole.sec.analyze': 'Dung lượng ổ đĩa',
    'mole.analyze.title': 'Cái gì chiếm chỗ',
    'mole.analyze.desc': 'Thư mục lớn nhất xếp trước. Chọn một thư mục để xem bên trong.',
    'mole.analyze.scan': 'Quét máy này',
    'mole.analyze.up': 'Lên trên',
    'mole.analyze.empty': 'Hãy quét để biết dung lượng đã đi đâu.',
    'mole.sec.history': 'Lịch sử',
    'mole.history.title': 'Các lần chạy gần đây',
    'mole.history.desc': 'Những gì Mole đã làm gần đây, theo nhật ký của chính nó.',
    'mole.history.scan': 'Xem lịch sử',
    'mole.history.empty': 'Chưa có gì để hiện.',
    'mole.history.items': '{n} mục',
    'mole.task.purge': 'xóa thư mục build', 'mole.task.uninstall': 'gỡ ứng dụng',
    'fault.mole_missing': 'Chưa cài Mole.',
    'fault.mole_terminal': 'Không mở được Terminal.',
  };

  const size = (n) => {
    if (!(n > 0)) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.min(units.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
    return (n / 1024 ** i).toFixed(i > 1 ? 1 : 0) + ' ' + units[i];
  };
  const mole = () => S.mole;
  const busy = () => !!mole().busy;
  const act = (name) => send({ t: 'action', name });

  function meter(key, value, detail) {
    const fill = h('span');
    const bar = h('div', { class: 'mole-bar', role: 'progressbar', 'aria-valuemin': '0', 'aria-valuemax': '100', 'aria-label': t('mole.' + key) }, fill);
    const num = h('b');
    const extra = h('span');
    reg(() => {
      const v = Math.max(0, Math.min(100, +value() || 0));
      fill.style.width = v + '%';
      bar.classList.toggle('hot', v >= 90);
      bar.setAttribute('aria-valuenow', String(Math.round(v)));
      num.textContent = Math.round(v) + '%';
      extra.textContent = detail ? detail() : '';
    });
    return h('div', {}, h('div', { class: 'mole-meter-label' }, h('span', { text: t('mole.' + key) }), num), bar, h('div', { class: 'mole-meter-label' }, extra));
  }

  function healthCards() {
    const st = () => mole().status || {};
    const score = h('span', { class: 'mole-score' });
    const refresh = button(t('mole.health.refresh'), () => act('mole_status'), { icon: 'refresh', small: true });
    reg(() => {
      score.textContent = st().score != null ? String(st().score) : '';
      refresh.disabled = busy();
    });
    const head = row(t('mole.health.title'), null, [score, refresh]);
    const text = head.querySelector('.row-desc') || head.querySelector('.row-text').appendChild(h('div', { class: 'row-desc' }));
    reg(() => { text.textContent = st().score != null ? [st().message, st().chip, st().os].filter(Boolean).join(' / ') : t('mole.health.none'); });
    const meters = h('div', { class: 'mole-meters' },
      meter('cpu', () => st().cpu),
      meter('memory', () => st().memory, () => (st().swap_total ? t('mole.swap', { used: size(st().swap_used), total: size(st().swap_total) }) : '')),
      meter('disk', () => st().disk, () => (st().disk_total ? size(st().disk_used) + ' / ' + size(st().disk_total) : '')));
    reg(() => { meters.style.display = st().score != null ? '' : 'none'; });
    return [sectionTitle(t('mole.sec.health')), card(head, meters)];
  }

  function taskCard(task) {
    const preview = button(t('mole.preview'), () => act('mole_' + task + '_preview'), { icon: 'magnifyingglass', small: true });
    const run = button(t('mole.' + task + '.run'), () => confirmDialog(
      { title: t('mole.' + task + '.confirm.title'), body: t('mole.' + task + '.confirm.body'), confirm: t('mole.' + task + '.run') },
      () => act('mole_' + task + '_run')), { small: true, danger: task === 'clean' });
    reg(() => { preview.disabled = run.disabled = busy(); });
    return [sectionTitle(t('mole.sec.' + task)), card(row(t('mole.' + task + '.title'), t('mole.' + task + '.desc'), h('div', { class: 'mole-actions' }, preview, run)))];
  }

  function reportCard() {
    const box = h('div');
    let last = null;
    reg(() => {
      const r = mole().report;
      const stamp = JSON.stringify([r, mole().busy, lang]);
      if (stamp === last) return;
      last = stamp;
      if (mole().busy) { box.replaceChildren(h('div', { class: 'empty', role: 'status', text: t('mole.working') })); return; }
      if (!r) { box.replaceChildren(h('div', { class: 'empty', text: t('mole.report.empty') })); return; }
      const task = t('mole.task.' + r.task);
      const kids = [h('div', { class: 'mole-summary' },
        h('div', { text: r.ok ? t(r.dry ? 'mole.report.preview' : 'mole.report.done', { task }) : t('mole.report.failed') + (r.error ? ': ' + r.error : '') }),
        (r.summary || []).map((s) => h('div', { text: s })))];
      for (const s of r.sections || []) {
        kids.push(h('div', { class: 'mole-section' }, h('div', { class: 'mole-section-name', text: s.name }),
          s.lines.map((l) => h('div', { class: 'mole-line ' + l.kind, text: l.text }))));
      }
      box.replaceChildren(...kids);
    });
    return [sectionTitle(t('mole.sec.report')), card(box)];
  }


  function uninstallCard() {
    const scan = button(t('mole.uninstall.scan'), () => act('mole_apps'), { icon: 'refresh', small: true });
    const list = h('div', { class: 'mole-list' });
    let last = null;
    reg(() => {
      scan.disabled = busy();
      const apps = mole().apps;
      const stamp = JSON.stringify([apps, mole().busy, lang]);
      if (stamp === last) return;
      last = stamp;
      if (!apps || !apps.length) { list.replaceChildren(h('div', { class: 'empty', text: t('mole.uninstall.empty') })); return; }
      list.replaceChildren(...apps.map((a) => {
        const preview = button(t('mole.preview'), () => act('mole_uninstall_preview:' + a.uninstall_name), { small: true });
        const run = button(t('mole.uninstall.run'), () => confirmDialog(
          { title: t('mole.uninstall.confirm.title', { name: a.name }), body: t('mole.uninstall.confirm.body'), confirm: t('mole.uninstall.run') },
          () => act('mole_uninstall_run:' + a.uninstall_name)), { small: true, danger: true });
        preview.disabled = run.disabled = !!mole().busy;
        return h('div', { class: 'mole-app' }, h('span', { class: 'mole-app-name', text: a.name }), h('span', { class: 'mole-app-size', text: a.size }), preview, run);
      }));
    });
    return [sectionTitle(t('mole.sec.uninstall')), card(row(t('mole.uninstall.title'), t('mole.uninstall.desc'), scan), list)];
  }

  function analyzeCard() {
    const scan = button(t('mole.analyze.scan'), () => act('mole_analyze'), { icon: 'magnifyingglass', small: true });
    const box = h('div');
    let last = null;
    const parent = (path) => path.replace(/\/[^/]+\/?$/, '') || '/';
    reg(() => {
      scan.disabled = busy();
      const a = mole().analyze;
      const stamp = JSON.stringify([a, mole().busy, lang]);
      if (stamp === last) return;
      last = stamp;
      if (!a || !a.entries) { box.replaceChildren(h('div', { class: 'empty', text: t('mole.analyze.empty') })); return; }
      const top = Math.max(1, ...a.entries.map((e) => e.size));
      const head = h('div', { class: 'mole-summary' }, h('div', { text: a.path + ' (' + size(a.total || a.entries.reduce((n, e) => n + e.size, 0)) + ')' }));
      if (!a.overview && a.path !== '/') head.append(button(t('mole.analyze.up'), () => act('mole_analyze:' + parent(a.path)), { small: true }));
      box.replaceChildren(head, ...a.entries.map((e) => {
        const fill = h('span');
        fill.style.width = Math.max(1, (e.size / top) * 100) + '%';
        const label = h('span', { class: 'mole-entry-name', text: e.name }, );
        const el = h(e.dir ? 'button' : 'div', { class: 'mole-entry' + (e.dir ? ' dir' : ''), 'aria-label': e.name + ', ' + size(e.size) },
          label, h('span', { class: 'mole-entry-size', text: size(e.size) }), h('div', { class: 'mole-bar' }, fill));
        if (e.dir) el.addEventListener('click', () => act('mole_analyze:' + e.path));
        return el;
      }));
    });
    return [sectionTitle(t('mole.sec.analyze')), card(row(t('mole.analyze.title'), t('mole.analyze.desc'), scan), box)];
  }

  function historyCard() {
    const scan = button(t('mole.history.scan'), () => act('mole_history'), { icon: 'refresh', small: true });
    const box = h('div');
    let last = null;
    reg(() => {
      scan.disabled = busy();
      const rows = mole().history;
      const stamp = JSON.stringify([rows, lang]);
      if (stamp === last) return;
      last = stamp;
      if (!rows || !rows.length) { box.replaceChildren(h('div', { class: 'empty', text: t('mole.history.empty') })); return; }
      box.replaceChildren(...rows.map((r) => h('div', { class: 'mole-app' },
        h('span', { class: 'mole-app-name', text: r.command }), h('span', { class: 'mole-app-size', text: t('mole.history.items', { n: r.items }) + ' / ' + r.size }),
        h('span', { class: 'mole-line', text: r.at }))));
    });
    return [sectionTitle(t('mole.sec.history')), card(row(t('mole.history.title'), t('mole.history.desc'), scan), box)];
  }

  function installerCard() {
    return [sectionTitle(t('mole.sec.installer')), card(row(t('mole.installer.title'), t('mole.installer.desc'),
      button(t('mole.installer.open'), () => act('mole_terminal_installer'), { icon: 'terminal.fill', small: true })))];
  }

  function missingCard() {
    return card(row(t('mole.missing.title'), t('mole.missing.desc'), button(t('mole.site'), () => act('mole_open_site'), { small: true })));
  }

  (window.__plugins = window.__plugins || []).push({
    id: 'mole',
    icon: 'sparkles',
    en,
    vi,
    build() {
      const ready = [...healthCards(), ...reportCard(), ...taskCard('clean'), ...taskCard('optimize'), ...taskCard('purge'), ...uninstallCard(), ...analyzeCard(), ...installerCard(), ...historyCard()];
      const missing = missingCard();
      const wrap = h('div', {}, ...ready);
      reg(() => {
        wrap.style.display = mole().installed ? '' : 'none';
        missing.style.display = mole().installed ? 'none' : '';
      });
      if (mole().installed) act('mole_status');
      return [missing, wrap];
    },
    selftest({ expect, go, sent, S }) {
      S.mole.status = { score: 91, message: 'Excellent', uptime: '12d 0h', model: 'Mac mini', chip: 'Apple M4', os: 'macOS 27', cpu: 13.6, memory: 81.2, swap_used: 3858169856, swap_total: 5368709120, disk: 48.2, disk_used: 238313164359, disk_total: 494384795648 };
      S.mole.report = { task: 'clean', dry: true, ok: true, sections: [{ name: 'User essentials', lines: [{ kind: 'item', text: 'User app cache' }] }], summary: ['Dry run complete'] };
      sent.length = 0;
      go('mole');
      expect(sent.some((m) => m.t === 'action' && m.name === 'mole_status'), 'mole: opening the page reads the health: ' + JSON.stringify(sent));
      const btn = (label) => [...document.querySelectorAll('.btn')].find((b) => b.textContent === label);
      sent.length = 0;
      btn(t('mole.preview')).click();
      expect(sent.some((m) => m.name === 'mole_clean_preview'), 'mole: the first preview is the clean one: ' + JSON.stringify(sent));
      sent.length = 0;
      btn(t('mole.clean.run')).click();
      expect(!sent.length, 'mole: cleaning for real asks first');
      const yes = document.querySelector('#modal .btn.primary, #modal .btn.danger');
      expect(!!yes, 'mole: the confirmation has a button');
      if (yes) yes.click();
      expect(sent.some((m) => m.name === 'mole_clean_run'), 'mole: confirming sends the run: ' + JSON.stringify(sent));
      expect(document.body.textContent.includes('User essentials'), 'mole: the report lists its sections');
      S.mole.apps = [{ name: 'Figma', uninstall_name: 'Figma', size: '325.7MB' }];
      S.mole.analyze = { path: '/Users/me', overview: false, total: 300, entries: [{ name: 'Library', path: '/Users/me/Library', size: 200, dir: true }, { name: 'a.txt', path: '/Users/me/a.txt', size: 100, dir: false }] };
      S.mole.history = [{ command: 'clean', at: '2026-10-19 10:00:00', items: 3, size: '1MB', failed: 0 }];
      runSyncs();
      sent.length = 0;
      btn(t('mole.uninstall.run')).click();
      const ok = document.querySelector('#modal .btn.primary, #modal .btn.danger');
      if (ok) ok.click();
      expect(sent.some((m) => m.name === 'mole_uninstall_run:Figma'), 'mole: uninstalling names the app after the confirmation: ' + JSON.stringify(sent));
      sent.length = 0;
      document.querySelector('.mole-entry.dir').click();
      expect(sent.some((m) => m.name === 'mole_analyze:/Users/me/Library'), 'mole: a folder opens when chosen: ' + JSON.stringify(sent));
      expect(document.body.textContent.includes('2026-10-19'), 'mole: the history is listed');
      S.mole.apps = null; S.mole.analyze = null; S.mole.history = null; runSyncs();
      S.mole.busy = 'clean'; runSyncs();
      expect(btn(t('mole.preview')).disabled, 'mole: buttons are off while it works');
      S.mole.busy = null; runSyncs();
      S.mole.installed = false; runSyncs();
      expect(!!btn(t('mole.site')), 'mole: the missing card offers the site');
      S.mole.installed = true; S.mole.status = null; S.mole.report = null; runSyncs();
    },
  });
})();
