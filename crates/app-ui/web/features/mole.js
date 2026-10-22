// Mole: the health of this Mac and what can be cleaned from it, after the features of Mole (github.com/tw93/Mole)
// but done by the app itself. The state is under S.mole; the page scans as it opens and every list can be chosen
// from and removed together. Every removal is checked again by the app.
(function () {
  const en = {
    'nav.mole': 'Mole',
    'page.mole': 'Mole',
    'page.mole.sub': 'The health of this Mac and what can be cleaned from it. Lists fill in by themselves; choose rows and remove them together.',
    'mole.refresh': 'Refresh', 'mole.refresh.all': 'Refresh all',
    'mole.sec.health': 'Health',
    'mole.health.title': 'Health score',
    'mole.health.reading': 'Reading',
    'mole.health.good': 'Running well', 'mole.health.fair': 'Could use some room', 'mole.health.poor': 'Under strain',
    'mole.health.can_free': '{size} can be cleaned',
    'mole.cpu': 'CPU', 'mole.memory': 'Memory', 'mole.disk': 'Disk',
    'mole.swap': 'Swap {used} of {total}',
    'mole.sec.clean': 'Caches and logs',
    'mole.clean.title': 'Free up disk space',
    'mole.clean.desc': 'App and browser caches, logs, developer caches and the Trash. They are emptied, not moved to the Trash; apps make them again when needed.',
    'mole.clean.empty': 'Nothing to clean.',
    'mole.clean.remove': 'Clean selected',
    'mole.clean.body': 'The contents of the chosen folders are deleted for good. Apps make their caches again when needed.',
    'mole.group.cache': 'Cache', 'mole.group.log': 'Logs', 'mole.group.dev': 'Developer', 'mole.group.trash': 'Trash',
    'mole.sec.purge': 'Old build folders',
    'mole.purge.title': 'Build folders of projects',
    'mole.purge.desc': 'node_modules, target, build, .venv, bin and obj and similar folders that their tools make again, found in the usual project folders.',
    'mole.purge.empty': 'No build folders found.',
    'mole.purge.remove': 'Delete selected',
    'mole.purge.body': 'The chosen build folders are deleted for good. Building the project makes them again.',
    'mole.age': 'touched {n} days ago', 'mole.age.today': 'touched today',
    'mole.sec.installers': 'Installers',
    'mole.installers.title': 'Old installer files',
    'mole.installers.desc': '.dmg, .pkg, .iso and .xip files in Downloads, Desktop and Documents.',
    'mole.installers.empty': 'No installer files found.',
    'mole.installers.remove': 'Move to Trash',
    'mole.installers.body': 'The chosen files go to the Trash, where they can be put back.',
    'mole.sec.apps': 'Uninstall apps',
    'mole.apps.title': 'Apps with their leftovers',
    'mole.apps.desc': 'Each app and what it left in Library go to the Trash together, so they can be put back. Quit an app before removing it.',
    'mole.apps.empty': 'No apps found.',
    'mole.apps.remove': 'Uninstall selected',
    'mole.apps.body': 'The chosen apps and their leftover files go to the Trash.',
    'mole.apps.extra': '{n} leftovers',
    'mole.sec.analyze': 'Disk usage',
    'mole.analyze.title': 'What takes the space',
    'mole.analyze.desc': 'The biggest entries first. Open a folder to look inside it; what is chosen goes to the Trash.',
    'mole.analyze.empty': 'This folder is empty.',
    'mole.analyze.remove': 'Move to Trash',
    'mole.analyze.body': 'The chosen files and folders go to the Trash, where they can be put back.',
    'mole.analyze.other': 'Other', 'mole.analyze.chart': 'Disk usage of {path}: {total}. Largest: {top}.',
    'mole.analyze.up': 'Up', 'mole.analyze.open': 'Open {name}', 'mole.analyze.home': 'Home',
    'mole.scanning': 'Scanning...',
    'mole.search': 'Filter',
    'mole.pick.all': 'Choose all',
    'mole.total': '{n} items, {size}',
    'mole.picked': '{n} chosen, {size}',
    'mole.confirm': 'Remove {n} items ({size})?',
    'mole.result': 'Removed {n} items, {size} freed.',
    'mole.result.partial': 'Removed {n} items, {size} freed. {failed} could not be removed.',
  };
  const vi = {
    'nav.mole': 'Mole',
    'page.mole': 'Mole',
    'page.mole.sub': 'Sức khỏe của máy Mac và những gì có thể dọn. Danh sách tự hiện ra; chọn nhiều dòng rồi xóa cùng lúc.',
    'mole.refresh': 'Làm mới', 'mole.refresh.all': 'Làm mới tất cả',
    'mole.sec.health': 'Sức khỏe',
    'mole.health.title': 'Điểm sức khỏe',
    'mole.health.reading': 'Đang đọc',
    'mole.health.good': 'Máy chạy tốt', 'mole.health.fair': 'Nên dọn bớt', 'mole.health.poor': 'Máy đang quá tải',
    'mole.health.can_free': 'Có thể dọn {size}',
    'mole.cpu': 'CPU', 'mole.memory': 'Bộ nhớ', 'mole.disk': 'Ổ đĩa',
    'mole.swap': 'Swap {used} trên {total}',
    'mole.sec.clean': 'Bộ nhớ đệm và nhật ký',
    'mole.clean.title': 'Giải phóng dung lượng',
    'mole.clean.desc': 'Bộ nhớ đệm của ứng dụng và trình duyệt, nhật ký, bộ nhớ đệm cho lập trình và Thùng rác. Chúng bị xóa hẳn, không vào Thùng rác; ứng dụng sẽ tự tạo lại khi cần.',
    'mole.clean.empty': 'Không có gì để dọn.',
    'mole.clean.remove': 'Dọn mục đã chọn',
    'mole.clean.body': 'Nội dung của các thư mục đã chọn bị xóa hẳn. Ứng dụng sẽ tự tạo lại bộ nhớ đệm khi cần.',
    'mole.group.cache': 'Bộ nhớ đệm', 'mole.group.log': 'Nhật ký', 'mole.group.dev': 'Lập trình', 'mole.group.trash': 'Thùng rác',
    'mole.sec.purge': 'Thư mục build cũ',
    'mole.purge.title': 'Thư mục build của dự án',
    'mole.purge.desc': 'node_modules, target, build, .venv, bin, obj và các thư mục tương tự mà công cụ dựng lại được, tìm trong các thư mục dự án thông dụng.',
    'mole.purge.empty': 'Không thấy thư mục build nào.',
    'mole.purge.remove': 'Xóa mục đã chọn',
    'mole.purge.body': 'Các thư mục build đã chọn bị xóa hẳn. Build lại dự án sẽ tạo lại chúng.',
    'mole.age': 'sửa {n} ngày trước', 'mole.age.today': 'sửa hôm nay',
    'mole.sec.installers': 'Bộ cài',
    'mole.installers.title': 'Tệp cài đặt cũ',
    'mole.installers.desc': 'Các tệp .dmg, .pkg, .iso và .xip trong Downloads, Desktop và Documents.',
    'mole.installers.empty': 'Không thấy tệp cài đặt nào.',
    'mole.installers.remove': 'Chuyển vào Thùng rác',
    'mole.installers.body': 'Các tệp đã chọn được chuyển vào Thùng rác nên có thể khôi phục.',
    'mole.sec.apps': 'Gỡ ứng dụng',
    'mole.apps.title': 'Ứng dụng cùng phần còn sót',
    'mole.apps.desc': 'Mỗi ứng dụng và những gì nó để lại trong Library được chuyển vào Thùng rác cùng lúc nên có thể khôi phục. Hãy thoát ứng dụng trước khi gỡ.',
    'mole.apps.empty': 'Không thấy ứng dụng nào.',
    'mole.apps.remove': 'Gỡ mục đã chọn',
    'mole.apps.body': 'Các ứng dụng đã chọn và tệp còn sót được chuyển vào Thùng rác.',
    'mole.apps.extra': '{n} tệp còn sót',
    'mole.sec.analyze': 'Dung lượng ổ đĩa',
    'mole.analyze.title': 'Cái gì chiếm chỗ',
    'mole.analyze.desc': 'Mục lớn nhất xếp trước. Mở một thư mục để xem bên trong; mục đã chọn được chuyển vào Thùng rác.',
    'mole.analyze.empty': 'Thư mục này trống.',
    'mole.analyze.remove': 'Chuyển vào Thùng rác',
    'mole.analyze.body': 'Các tệp và thư mục đã chọn được chuyển vào Thùng rác nên có thể khôi phục.',
    'mole.analyze.other': 'Khác', 'mole.analyze.chart': 'Dung lượng của {path}: {total}. Lớn nhất: {top}.',
    'mole.analyze.up': 'Lên trên', 'mole.analyze.open': 'Mở {name}', 'mole.analyze.home': 'Thư mục nhà',
    'mole.scanning': 'Đang quét...',
    'mole.search': 'Lọc',
    'mole.pick.all': 'Chọn tất cả',
    'mole.total': '{n} mục, {size}',
    'mole.picked': 'Đã chọn {n} mục, {size}',
    'mole.confirm': 'Xóa {n} mục ({size})?',
    'mole.result': 'Đã xóa {n} mục, giải phóng {size}.',
    'mole.result.partial': 'Đã xóa {n} mục, giải phóng {size}. Còn {failed} mục không xóa được.',
  };

  const size = (n) => {
    if (!(n > 0)) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.min(units.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
    return (n / 1024 ** i).toFixed(i > 1 ? 1 : 0) + ' ' + units[i];
  };
  const sum = (list) => list.reduce((n, i) => n + i.size, 0);
  const mole = () => S.mole;
  const busy = (key) => (mole().busy || []).includes(key);
  const act = (name) => send({ t: 'action', name });
  const tilde = (p) => (mole().home && p.startsWith(mole().home) ? '~' + p.slice(mole().home.length) : p);
  const scanning = () => h('div', { class: 'empty', role: 'status', text: t('mole.scanning') });

  function meter(key, value, detail) {
    const fill = h('span');
    const bar = h('div', { class: 'mole-bar', role: 'progressbar', 'aria-valuemin': '0', 'aria-valuemax': '100', 'aria-label': t('mole.' + key) }, fill);
    const num = h('b');
    const extra = h('span');
    reg(() => {
      const v = clamp(+value() || 0, 0, 100);
      fill.style.width = v + '%';
      bar.classList.toggle('hot', v >= 90);
      bar.setAttribute('aria-valuenow', String(Math.round(v)));
      num.textContent = Math.round(v) + '%';
      extra.textContent = detail() || '';
    });
    return h('div', {}, h('div', { class: 'mole-meter-label' }, h('span', { text: t('mole.' + key) }), num), bar, h('div', { class: 'mole-meter-label' }, extra));
  }

  function healthCard() {
    const st = () => mole().status || {};
    const score = h('span', { class: 'mole-score' });
    const refresh = button(t('mole.refresh.all'), () => act('mole_refresh'), { icon: 'refresh', small: true });
    const head = row(t('mole.health.title'), ' ', h('div', { class: 'mole-actions' }, score, refresh));
    const desc = head.querySelector('.row-desc');
    reg(() => {
      const s = st().score;
      score.textContent = s != null ? String(s) : '';
      score.className = 'mole-score ' + (s >= 85 ? 'good' : s >= 60 ? 'fair' : 'poor');
      refresh.disabled = busy('status');
      const lists = ['clean', 'purge', 'installers'].map((k) => mole()[k] || []);
      const free = lists.reduce((n, l) => n + sum(l), 0);
      const word = s == null ? t('mole.health.reading') : t('mole.health.' + (s >= 85 ? 'good' : s >= 60 ? 'fair' : 'poor'));
      desc.textContent = free ? word + ' / ' + t('mole.health.can_free', { size: size(free) }) : word;
    });
    const meters = h('div', { class: 'mole-meters' },
      meter('cpu', () => st().cpu, () => ''),
      meter('memory', () => st().memory, () => (st().swap_total ? t('mole.swap', { used: size(st().swap_used), total: size(st().swap_total) }) : '')),
      meter('disk', () => st().disk, () => (st().disk_total ? size(st().disk_used) + ' / ' + size(st().disk_total) : '')));
    return [sectionTitle(t('mole.sec.health')), card(head, meters)];
  }

  /**
   * A list whose rows are chosen with a checkbox and removed together. `detail` is the second line of a row,
   * `lead` an element before its name, `open` makes a folder row open; `head` goes above the toolbar.
   */
  function pickList({ kind, read, detail, lead, open, head, filter }) {
    const chosen = new Set();
    const items = () => read() || [];
    const all = h('input', { type: 'checkbox', 'aria-label': t('mole.pick.all') });
    const count = h('span', { class: 'mole-count' });
    const search = filter ? h('input', { type: 'search', class: 'text-field mole-search', placeholder: t('mole.search'), 'aria-label': t('mole.search'), spellcheck: 'false', autocomplete: 'off' }) : null;
    const remove = button(t('mole.' + kind + '.remove'), () => {
      const picked = items().filter((i) => chosen.has(i.path));
      confirmDialog({ title: t('mole.confirm', { n: picked.length, size: size(sum(picked)) }), body: t('mole.' + kind + '.body'), confirm: t('mole.' + kind + '.remove') },
        () => act('mole_remove:' + kind + '\n' + picked.map((i) => i.path).join('\n')));
    }, { icon: 'trash', small: true, danger: true });
    const refresh = button('', () => act('mole_scan:' + kind), { icon: 'refresh', small: true, aria: t('mole.refresh') });
    refresh.title = t('mole.refresh');
    const box = h('div', { class: 'mole-list' });
    const visible = () => {
      const q = search ? search.value.trim().toLowerCase() : '';
      return items().filter((i) => !q || (i.name + ' ' + i.note).toLowerCase().includes(q));
    };
    const paint = () => {
      const list = items();
      for (const p of [...chosen]) if (!list.some((i) => i.path === p)) chosen.delete(p);
      const picked = list.filter((i) => chosen.has(i.path));
      const shown = visible();
      const n = shown.filter((i) => chosen.has(i.path)).length;
      all.checked = shown.length > 0 && n === shown.length;
      all.indeterminate = n > 0 && n < shown.length;
      all.disabled = !shown.length;
      count.textContent = busy(kind) || !read() ? t('mole.scanning')
        : picked.length ? t('mole.picked', { n: picked.length, size: size(sum(picked)) })
        : t('mole.total', { n: list.length, size: size(sum(list)) });
      remove.disabled = !picked.length || busy('remove');
      refresh.disabled = busy(kind);
    };
    const draw = () => {
      if (!read()) { box.replaceChildren(scanning()); return; }
      const shown = visible();
      if (!shown.length) { box.replaceChildren(busy(kind) ? scanning() : h('div', { class: 'empty', text: t('mole.' + kind + '.empty') })); return; }
      const top = Math.max(1, ...shown.map((i) => i.size));
      box.replaceChildren(...shown.map((i) => {
        const pick = h('input', { type: 'checkbox', 'aria-label': i.name });
        pick.checked = chosen.has(i.path);
        pick.addEventListener('change', () => { if (pick.checked) chosen.add(i.path); else chosen.delete(i.path); paint(); });
        const fill = h('span');
        fill.style.width = Math.max(1, (i.size / top) * 100) + '%';
        const go = open && i.dir ? button('', (e) => { e.preventDefault(); open(i); }, { icon: 'folder', small: true, aria: t('mole.analyze.open', { name: i.name }) }) : null;
        return h('label', { class: 'mole-item', title: tilde(i.path), 'data-path': i.path }, pick, lead ? lead(i) : null,
          h('span', { class: 'mole-item-text' }, h('span', { class: 'mole-item-name', text: i.name }), h('span', { class: 'mole-item-note', text: detail(i) }),
            h('span', { class: 'mole-bar' }, fill)),
          h('span', { class: 'mole-item-size', text: size(i.size) }), go);
      }));
    };
    all.addEventListener('change', () => {
      for (const i of visible()) { if (all.checked) chosen.add(i.path); else chosen.delete(i.path); }
      draw(); paint();
    });
    if (search) search.addEventListener('input', () => { draw(); paint(); });
    let last = null;
    reg(() => {
      const stamp = JSON.stringify([read(), busy(kind), lang]);
      if (stamp !== last) { last = stamp; draw(); }
      paint();
    });
    return [sectionTitle(t('mole.sec.' + kind)), card(row(t('mole.' + kind + '.title'), t('mole.' + kind + '.desc'), null), head,
      h('div', { class: 'mole-toolbar' }, h('label', { class: 'mole-all' }, all, count), search, remove, refresh), box)];
  }

  const age = (i) => (i.age == null ? '' : i.age ? t('mole.age', { n: i.age }) : t('mole.age.today'));

  const MAX_SLICES = 12;
  const MIN_SHARE = 0.01;

  /** The slices of the ring: the biggest entries, then one "Other" for the rest and for what is too thin to see. */
  function slices(a) {
    const list = [...a.entries].filter((i) => i.size > 0).sort((x, y) => y.size - x.size);
    const whole = Math.max(a.total || 0, sum(list));
    if (!list.length || !whole) return [];
    const out = [];
    let rest = whole;
    for (const i of list) {
      if (out.length >= MAX_SLICES - 1 || i.size / whole < MIN_SHARE) break;
      out.push({ entry: i, name: i.name, size: i.size });
      rest -= i.size;
    }
    if (rest > 0) out.push({ entry: null, name: t('mole.analyze.other'), size: rest });
    return out.map((x) => ({ ...x, share: x.size / whole }));
  }

  const R_OUT = 90;
  const R_IN = 58;
  const point = (r, turn) => (100 + r * Math.sin(turn * 2 * Math.PI)).toFixed(2) + ' ' + (100 - r * Math.cos(turn * 2 * Math.PI)).toFixed(2);
  function arc(from, to) {
    const end = Math.min(to, from + 0.9999);
    const big = end - from > 0.5 ? 1 : 0;
    return 'M' + point(R_OUT, from) + 'A' + R_OUT + ' ' + R_OUT + ' 0 ' + big + ' 1 ' + point(R_OUT, end)
      + 'L' + point(R_IN, end) + 'A' + R_IN + ' ' + R_IN + ' 0 ' + big + ' 0 ' + point(R_IN, from) + 'Z';
  }
  const esc = (v) => String(v).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);

  /** The ring above the list. The list stays the way in for the keyboard, so the slices are not tab stops. */
  function analyzeChart() {
    const wrap = h('div', { class: 'mole-chart' });
    let cut = [];
    let last = null;
    const rowOf = (path) => [...(wrap.closest('.card') || document).querySelectorAll('.mole-item')].find((r) => r.dataset.path === path);
    const light = (k) => {
      wrap.querySelectorAll('.mole-slice').forEach((el) => el.classList.toggle('lit', el.dataset.k === k));
      const x = cut[k];
      const name = wrap.querySelector('.mole-chart-name');
      const num = wrap.querySelector('.mole-chart-size');
      const a = mole().analyze;
      name.textContent = x ? x.name : tilde(a.path).split('/').pop() || '/';
      num.textContent = x ? size(x.size) + ' / ' + Math.round(x.share * 100) + '%' : size(a.total);
      wrap.querySelectorAll('.mole-item.lit').forEach((r) => r.classList.remove('lit'));
      const row = x && x.entry && rowOf(x.entry.path);
      if (row) row.classList.add('lit');
    };
    wrap.addEventListener('mouseover', (e) => { const el = e.target.closest('.mole-slice'); if (el) light(el.dataset.k); });
    wrap.addEventListener('mouseout', (e) => { if (e.target.closest('.mole-slice')) light(null); });
    wrap.addEventListener('click', (e) => {
      const el = e.target.closest('.mole-slice');
      const x = el && cut[el.dataset.k];
      if (!x || !x.entry) return;
      if (x.entry.dir) { act('mole_analyze:' + x.entry.path); return; }
      const row = rowOf(x.entry.path);
      const pick = row && row.querySelector('input');
      if (pick && !pick.checked) pick.click();
      if (row) row.scrollIntoView({ block: 'nearest' });
    });
    reg(() => {
      const a = mole().analyze;
      cut = a && !busy('analyze') ? slices(a) : [];
      const stamp = JSON.stringify([cut.map((x) => [x.name, x.size]), a && a.path, a && a.total, lang]);
      if (stamp === last) return;
      last = stamp;
      wrap.hidden = !cut.length;
      if (!cut.length) { wrap.replaceChildren(); return; }
      let at = 0;
      const paths = cut.map((x, k) => {
        const d = arc(at, at + x.share);
        at += x.share;
        const fill = x.entry ? 'hsl(' + Math.round((k * 360) / MAX_SLICES + 215) % 360 + ' 62% 56%)' : 'var(--text-2)';
        return '<path class="mole-slice' + (x.entry && !x.entry.dir ? ' file' : '') + '" data-k="' + k + '" d="' + d + '" style="fill:' + fill + '"></path>';
      }).join('');
      const top = cut.filter((x) => x.entry).slice(0, 3).map((x) => x.name + ' ' + size(x.size)).join(', ');
      wrap.innerHTML = '<svg viewBox="0 0 200 200" role="img" aria-label="'
        + esc(t('mole.analyze.chart', { path: tilde(a.path), total: size(a.total), top })) + '">' + paths
        + '<foreignObject x="46" y="70" width="108" height="60"><div xmlns="http://www.w3.org/1999/xhtml" class="mole-chart-mid" aria-hidden="true">'
        + '<span class="mole-chart-name"></span><span class="mole-chart-size"></span></div></foreignObject></svg>';
      light(null);
    });
    return wrap;
  }

  function analyzeHead() {
    const where = h('span', { class: 'mole-path' });
    const up = button(t('mole.analyze.up'), () => act('mole_analyze:' + (mole().analyze.path.replace(/\/[^/]+\/?$/, '') || '/')), { icon: 'chevron.up', small: true });
    const home = button(t('mole.analyze.home'), () => act('mole_analyze:' + mole().home), { small: true });
    reg(() => {
      const a = mole().analyze;
      where.textContent = a ? tilde(a.path) + ' (' + size(a.total) + ')' : '';
      up.disabled = !a || a.path === '/' || busy('analyze');
      home.disabled = !a || a.path === mole().home || busy('analyze');
    });
    return h('div', {}, analyzeChart(), h('div', { class: 'mole-toolbar' }, where, up, home));
  }

  (window.__plugins = window.__plugins || []).push({
    id: 'mole',
    icon: 'sparkles',
    en,
    vi,
    build() {
      let seen = mole().result ? mole().result.seq : null;
      reg(() => {
        const r = mole().result;
        if (!r || r.seq === seen) return;
        seen = r.seq;
        toast(t(r.failed ? 'mole.result.partial' : 'mole.result', { n: r.removed, size: size(r.freed), failed: r.failed }));
      });
      act('mole_open');
      return [
        ...healthCard(),
        ...pickList({ kind: 'clean', read: () => mole().clean, detail: (i) => t('mole.group.' + i.note) + ' / ' + tilde(i.path) }),
        ...pickList({ kind: 'purge', read: () => mole().purge, detail: (i) => [i.note, age(i)].filter(Boolean).join(' / ') }),
        ...pickList({ kind: 'installers', read: () => mole().installers, detail: (i) => tilde(i.path) }),
        ...pickList({ kind: 'apps', read: () => mole().apps, filter: true, lead: (i) => iconImg(i.note),
          detail: (i) => (i.extra ? i.note + ' / ' + t('mole.apps.extra', { n: i.extra }) : i.note) }),
        ...pickList({ kind: 'analyze', read: () => mole().analyze && mole().analyze.entries, head: analyzeHead(), detail: () => '',
          open: (i) => act('mole_analyze:' + i.path) }),
      ];
    },
    selftest({ expect, go, sent, S }) {
      const home = S.mole.home;
      S.mole.status = { score: 91, cpu: 13.6, memory: 81.2, memory_used: 1, memory_total: 2, swap_used: 3858169856, swap_total: 5368709120, disk: 48.2, disk_used: 238313164359, disk_total: 494384795648 };
      S.mole.clean = [
        { path: home + '/Library/Caches/org.a', name: 'org.a', note: 'cache', size: 3000000, dir: true, age: 1, extra: 0 },
        { path: home + '/Library/Logs/b', name: 'b', note: 'log', size: 2000000, dir: true, age: 1, extra: 0 },
      ];
      S.mole.purge = [];
      S.mole.installers = null;
      S.mole.apps = [{ path: '/Applications/Figma.app', name: 'Figma', note: 'com.figma.Desktop', size: 400, dir: true, age: 3, extra: 2 },
        { path: '/Applications/Zed.app', name: 'Zed', note: 'dev.zed.Zed', size: 300, dir: true, age: 3, extra: 0 }];
      S.mole.analyze = { path: home, total: 300, entries: [{ path: home + '/Library', name: 'Library', note: '', size: 200, dir: true, age: 0, extra: 0 }] };
      sent.length = 0;
      go('mole');
      expect(sent.some((m) => m.t === 'action' && m.name === 'mole_open'), 'mole: opening the page scans by itself: ' + JSON.stringify(sent));
      expect(document.body.textContent.includes('org.a'), 'mole: the cleanable rows are shown without a click');
      expect(document.body.textContent.includes(t('mole.scanning')), 'mole: a list not read yet says it is scanning');
      const section = (kind) => [...document.querySelectorAll('.card')].find((c) => c.textContent.includes(t('mole.' + kind + '.title')));
      const btn = (root, label) => [...root.querySelectorAll('.btn')].find((b) => b.textContent === label);
      const clean = section('clean');
      const remove = btn(clean, t('mole.clean.remove'));
      expect(remove.disabled, 'mole: nothing chosen, nothing to remove');
      const all = clean.querySelector('.mole-all input');
      all.click();
      expect(!remove.disabled && clean.querySelectorAll('.mole-item input:checked').length === 2, 'mole: choose all picks every row');
      sent.length = 0;
      remove.click();
      expect(!sent.length, 'mole: removing asks first');
      const yes = document.querySelector('#modal .btn.primary');
      if (yes) yes.click();
      const msg = sent.find((m) => m.name && m.name.startsWith('mole_remove:'));
      expect(msg && msg.name === 'mole_remove:clean\n' + home + '/Library/Caches/org.a\n' + home + '/Library/Logs/b', 'mole: the chosen rows go together: ' + JSON.stringify(sent));
      const apps = section('apps');
      const search = apps.querySelector('.mole-search');
      search.value = 'zed';
      search.dispatchEvent(new Event('input'));
      expect(apps.querySelectorAll('.mole-item').length === 1, 'mole: the filter narrows the apps');
      apps.querySelector('.mole-all input').click();
      sent.length = 0;
      btn(apps, t('mole.apps.remove')).click();
      const ok = document.querySelector('#modal .btn.primary');
      if (ok) ok.click();
      expect(sent.some((m) => m.name === 'mole_remove:apps\n/Applications/Zed.app'), 'mole: only the filtered app is uninstalled: ' + JSON.stringify(sent));
      sent.length = 0;
      section('analyze').querySelector('.mole-item .btn').click();
      expect(sent.some((m) => m.name === 'mole_analyze:' + home + '/Library'), 'mole: a folder opens: ' + JSON.stringify(sent));
      const ring = () => section('analyze').querySelectorAll('.mole-slice');
      expect(ring().length === 2, 'mole: one entry and what is left make two slices');
      expect(section('analyze').querySelector('svg[role=img]').getAttribute('aria-label').includes('Library'), 'mole: the ring is described for a reader');
      sent.length = 0;
      ring()[0].dispatchEvent(new MouseEvent('click', { bubbles: true }));
      expect(sent.some((m) => m.name === 'mole_analyze:' + home + '/Library'), 'mole: a folder slice opens it: ' + JSON.stringify(sent));
      S.mole.analyze = { path: home, total: 1000, entries: Array.from({ length: 20 }, (_, n) => ({ path: home + '/f' + n, name: 'f' + n, note: '', size: 100 - n * 4, dir: n % 2 === 0, age: 0, extra: 0 })) };
      runSyncs();
      expect(ring().length === 12 && ring()[11].getAttribute('style').includes('var(--text-2)'), 'mole: twenty entries make eleven slices and Other: ' + ring().length);
      sent.length = 0;
      ring()[11].dispatchEvent(new MouseEvent('click', { bubbles: true }));
      ring()[1].dispatchEvent(new MouseEvent('click', { bubbles: true }));
      expect(!sent.length && section('analyze').querySelectorAll('.mole-item input:checked').length === 1
        && section('analyze').querySelector('.mole-item input:checked').closest('.mole-item').dataset.path === home + '/f1', 'mole: a file slice chooses its row, Other does nothing');
      S.mole.analyze = { path: home, total: 0, entries: [] }; runSyncs();
      expect(!ring().length, 'mole: an empty folder has no ring');
      S.mole.analyze = { path: home, total: 300, entries: [{ path: home + '/Library', name: 'Library', note: '', size: 200, dir: true, age: 0, extra: 0 }] }; runSyncs();
      sent.length = 0;
      btn(section('installers'), '').click();
      expect(sent.some((m) => m.name === 'mole_scan:installers'), 'mole: each list has its own refresh: ' + JSON.stringify(sent));
      sent.length = 0;
      btn(document, t('mole.refresh.all')).click();
      expect(sent.some((m) => m.name === 'mole_refresh'), 'mole: refresh all scans everything again');
      S.mole.busy = ['remove']; runSyncs();
      expect(btn(clean, t('mole.clean.remove')).disabled, 'mole: no second removal while one runs');
      S.mole.clean = []; S.mole.busy = []; runSyncs();
      expect(clean.textContent.includes(t('mole.clean.empty')), 'mole: an empty list says so');
      S.mole.status = null; S.mole.clean = null; S.mole.purge = null; S.mole.apps = null; S.mole.analyze = null; runSyncs();
    },
  });
})();
