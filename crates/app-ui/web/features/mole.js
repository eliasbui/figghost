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
    'mole.live': 'Live', 'mole.gpu': 'Graphics', 'mole.network': 'Network', 'mole.sensors': 'Heat and power',
    'mole.cores': '{slow} efficiency and {fast} performance cores', 'mole.cores.n': '{n} cores',
    'mole.mem.app': 'Apps', 'mole.mem.wired': 'Wired', 'mole.mem.compressed': 'Compressed',
    'mole.pressure.normal': 'Pressure normal', 'mole.pressure.warning': 'Pressure high', 'mole.pressure.critical': 'Pressure critical',
    'mole.down': 'Down', 'mole.up': 'Up',
    'mole.disk.free': '{size} free',
    'mole.thermal.nominal': 'Cool', 'mole.thermal.fair': 'Warm', 'mole.thermal.serious': 'Hot', 'mole.thermal.critical': 'Too hot',
    'mole.fan': 'Fan {n}', 'mole.battery': 'Battery health', 'mole.battery.cycles': '{n} cycles',
    'mole.free.title': 'What can be cleaned',
    'mole.top.cpu': 'Busiest on the processor', 'mole.top.memory': 'Holding the most memory', 'mole.top.none': 'Nothing busy.',
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
    'mole.sec.docker': 'Docker',
    'mole.docker.title': 'Docker resources',
    'mole.docker.desc': 'What Docker keeps: containers with their logs, images, volumes, networks and the build cache.',
    'mole.docker.missing': 'Docker is not installed.', 'mole.docker.down': 'Docker is not running. Start Docker Desktop and refresh.',
    'mole.docker.disk': 'Docker Desktop takes {size} on this Mac.',
    'mole.docker.reclaim': '{size} can be reclaimed',
    'mole.docker.t.Images': 'Images', 'mole.docker.t.Containers': 'Containers', 'mole.docker.t.Local Volumes': 'Volumes', 'mole.docker.t.Build Cache': 'Build cache', 'mole.docker.t.logs': 'Logs',
    'mole.docker.clear_logs': 'Clear all logs', 'mole.docker.clear_logs.body': 'The logs of every container are emptied for good. The containers keep running.',
    'mole.docker.unused': 'Remove unused', 'mole.docker.unused.body': 'Stopped containers, unused networks, dangling images and dangling build cache are removed. Volumes are kept.',
    'mole.docker.cache': 'Clear build cache', 'mole.docker.cache.body': 'All build cache that nothing uses is removed. The next build takes longer.',
    'mole.docker.done': 'Docker: {n} done.', 'mole.docker.done.partial': 'Docker: {n} done, {failed} failed.',
    'mole.docker.logs': 'logs {size}', 'mole.docker.used': 'used by {n}', 'mole.docker.unused.tag': 'not used', 'mole.docker.anonymous': 'anonymous',
    'mole.docker.start': 'Start', 'mole.docker.stop': 'Stop', 'mole.docker.restart': 'Restart', 'mole.docker.clean': 'Clear logs', 'mole.docker.remove': 'Remove',
    'mole.docker.rm.body': 'The chosen containers are removed, running ones are stopped first. Their volumes are kept.',
    'mole.docker.logs.body': 'The logs of the chosen containers are emptied for good.',
    'mole.docker.rmi.body': 'The chosen images are removed. An image a container uses cannot be removed.',
    'mole.docker.volume_rm.body': 'The chosen volumes and the data in them are deleted for good. A volume in use cannot be removed.',
    'mole.docker.network_rm.body': 'The chosen networks are removed. A network in use cannot be removed.',
    'mole.sec.docker_containers': 'Docker containers', 'mole.docker_containers.title': 'Containers', 'mole.docker_containers.desc': 'Start, stop, restart, clear the logs of or remove the chosen containers.', 'mole.docker_containers.empty': 'No containers.',
    'mole.sec.docker_images': 'Docker images', 'mole.docker_images.title': 'Images', 'mole.docker_images.desc': 'The size is what removing an image frees: the layers no other image shares.', 'mole.docker_images.empty': 'No images.',
    'mole.sec.docker_volumes': 'Docker volumes', 'mole.docker_volumes.title': 'Volumes', 'mole.docker_volumes.desc': 'Volumes hold data; removing one deletes it.', 'mole.docker_volumes.empty': 'No volumes.',
    'mole.sec.docker_networks': 'Docker networks', 'mole.docker_networks.title': 'Networks', 'mole.docker_networks.desc': 'Networks made by you or by Compose. The built-in ones are not listed.', 'mole.docker_networks.empty': 'No networks.',
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
    'mole.live': 'Trực tiếp', 'mole.gpu': 'Đồ họa', 'mole.network': 'Mạng', 'mole.sensors': 'Nhiệt và nguồn',
    'mole.cores': '{slow} lõi tiết kiệm và {fast} lõi hiệu năng', 'mole.cores.n': '{n} lõi',
    'mole.mem.app': 'Ứng dụng', 'mole.mem.wired': 'Hệ thống ghim', 'mole.mem.compressed': 'Bị nén',
    'mole.pressure.normal': 'Áp lực bình thường', 'mole.pressure.warning': 'Áp lực cao', 'mole.pressure.critical': 'Áp lực nguy hiểm',
    'mole.down': 'Tải xuống', 'mole.up': 'Tải lên',
    'mole.disk.free': 'Còn trống {size}',
    'mole.thermal.nominal': 'Mát', 'mole.thermal.fair': 'Ấm', 'mole.thermal.serious': 'Nóng', 'mole.thermal.critical': 'Quá nóng',
    'mole.fan': 'Quạt {n}', 'mole.battery': 'Độ chai pin', 'mole.battery.cycles': '{n} lần sạc',
    'mole.free.title': 'Có thể dọn',
    'mole.top.cpu': 'Ngốn CPU nhất', 'mole.top.memory': 'Ngốn bộ nhớ nhất', 'mole.top.none': 'Không có gì đang chạy nặng.',
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
    'mole.sec.docker': 'Docker',
    'mole.docker.title': 'Tài nguyên Docker',
    'mole.docker.desc': 'Những gì Docker giữ: container cùng log, image, volume, network và build cache.',
    'mole.docker.missing': 'Chưa cài Docker.', 'mole.docker.down': 'Docker chưa chạy. Mở Docker Desktop rồi làm mới.',
    'mole.docker.disk': 'Docker Desktop đang chiếm {size} trên máy Mac.',
    'mole.docker.reclaim': 'Có thể thu hồi {size}',
    'mole.docker.t.Images': 'Image', 'mole.docker.t.Containers': 'Container', 'mole.docker.t.Local Volumes': 'Volume', 'mole.docker.t.Build Cache': 'Build cache', 'mole.docker.t.logs': 'Log',
    'mole.docker.clear_logs': 'Xóa toàn bộ log', 'mole.docker.clear_logs.body': 'Log của mọi container bị xóa hẳn. Container vẫn tiếp tục chạy.',
    'mole.docker.unused': 'Dọn phần không dùng', 'mole.docker.unused.body': 'Xóa container đã dừng, network không dùng, image treo và build cache treo. Volume được giữ lại.',
    'mole.docker.cache': 'Xóa build cache', 'mole.docker.cache.body': 'Xóa toàn bộ build cache không dùng. Lần build sau sẽ lâu hơn.',
    'mole.docker.done': 'Docker: xong {n} mục.', 'mole.docker.done.partial': 'Docker: xong {n} mục, {failed} mục không làm được.',
    'mole.docker.logs': 'log {size}', 'mole.docker.used': '{n} container dùng', 'mole.docker.unused.tag': 'không dùng', 'mole.docker.anonymous': 'ẩn danh',
    'mole.docker.start': 'Chạy', 'mole.docker.stop': 'Dừng', 'mole.docker.restart': 'Khởi động lại', 'mole.docker.clean': 'Xóa log', 'mole.docker.remove': 'Xóa',
    'mole.docker.rm.body': 'Các container đã chọn bị xóa, container đang chạy sẽ bị dừng trước. Volume của chúng được giữ lại.',
    'mole.docker.logs.body': 'Log của các container đã chọn bị xóa hẳn.',
    'mole.docker.rmi.body': 'Các image đã chọn bị xóa. Image đang có container dùng thì không xóa được.',
    'mole.docker.volume_rm.body': 'Các volume đã chọn và dữ liệu trong đó bị xóa hẳn. Volume đang dùng thì không xóa được.',
    'mole.docker.network_rm.body': 'Các network đã chọn bị xóa. Network đang dùng thì không xóa được.',
    'mole.sec.docker_containers': 'Docker container', 'mole.docker_containers.title': 'Container', 'mole.docker_containers.desc': 'Chạy, dừng, khởi động lại, xóa log hoặc xóa các container đã chọn.', 'mole.docker_containers.empty': 'Không có container nào.',
    'mole.sec.docker_images': 'Docker image', 'mole.docker_images.title': 'Image', 'mole.docker_images.desc': 'Dung lượng là phần được giải phóng khi xóa: các lớp không image nào khác dùng chung.', 'mole.docker_images.empty': 'Không có image nào.',
    'mole.sec.docker_volumes': 'Docker volume', 'mole.docker_volumes.title': 'Volume', 'mole.docker_volumes.desc': 'Volume chứa dữ liệu; xóa volume là xóa dữ liệu trong đó.', 'mole.docker_volumes.empty': 'Không có volume nào.',
    'mole.sec.docker_networks': 'Docker network', 'mole.docker_networks.title': 'Network', 'mole.docker_networks.desc': 'Network do bạn hoặc Compose tạo. Network có sẵn không được liệt kê.', 'mole.docker_networks.empty': 'Không có network nào.',
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

  // The last readings of the live figures, kept across visits of the page.
  const HISTORY = 60;
  const hist = { at: null, cpu: [], memory: [], gpu: [], down: [], up: [] };
  function remember(st) {
    if (st.at == null || st.at === hist.at) return;
    hist.at = st.at;
    const add = (k, v) => { hist[k].push(+v || 0); if (hist[k].length > HISTORY) hist[k].shift(); };
    add('cpu', st.cpu); add('memory', st.memory); add('gpu', st.gpu); add('down', st.net_down); add('up', st.net_up);
  }
  const rate = (n) => size(n) + '/s';

  /** A filled line of readings, oldest on the left, against `top` (or the largest of them). */
  function spark(values, top) {
    const max = Math.max(top || 0, ...values, 1e-9);
    const step = 120 / Math.max(1, HISTORY - 1);
    const x0 = 120 - (values.length - 1) * step;
    const pts = values.map((v, i) => (x0 + i * step).toFixed(1) + ',' + (36 - (v / max) * 33).toFixed(1));
    if (pts.length < 2) return '';
    return '<path class="area" d="M' + x0.toFixed(1) + ',36L' + pts.join('L') + 'L120,36Z"/><path class="line" d="M' + pts.join('L') + '"/>';
  }
  const sparkEl = (cls) => h('div', { class: 'mole-spark ' + cls, 'aria-hidden': 'true' });

  /** A ring filled to `v` out of 100, its figure in the middle. */
  function gauge(cls, label) {
    const el = h('div', { class: 'mole-gauge ' + cls, role: 'meter', 'aria-valuemin': '0', 'aria-valuemax': '100', 'aria-label': label },
      h('b'), h('span', { text: label }));
    el.set = (v, text) => {
      const n = clamp(+v || 0, 0, 100);
      el.style.setProperty('--v', n);
      el.setAttribute('aria-valuenow', String(Math.round(n)));
      el.querySelector('b').textContent = text != null ? text : Math.round(n) + '%';
    };
    return el;
  }

  function tile(cls, title, ...kids) {
    const value = h('b', { class: 'mole-tile-value' });
    const sub = h('span', { class: 'mole-tile-sub' });
    const el = h('div', { class: 'mole-tile ' + cls }, h('div', { class: 'mole-tile-head' }, h('span', { text: title }), value), sub, ...kids);
    return Object.assign(el, { value, sub });
  }

  function healthCard() {
    const st = () => mole().status || {};
    const word = (s) => (s >= 85 ? 'good' : s >= 60 ? 'fair' : 'poor');
    const score = h('div', { class: 'mole-score-ring', role: 'meter', 'aria-valuemin': '0', 'aria-valuemax': '100', 'aria-label': t('mole.health.title') },
      h('b'), h('span', { text: t('mole.health.title') }));
    const status = h('div', { class: 'mole-hero-word' });
    const free = h('div', { class: 'mole-hero-free' });
    const refresh = button(t('mole.refresh.all'), () => act('mole_refresh'), { icon: 'refresh', small: true });
    const live = h('span', { class: 'mole-live', text: t('mole.live') });
    const rings = { cpu: gauge('cpu', t('mole.cpu')), memory: gauge('mem', t('mole.memory')), disk: gauge('disk', t('mole.disk')), gpu: gauge('gpu', t('mole.gpu')) };
    const hero = h('div', { class: 'mole-hero' }, score,
      h('div', { class: 'mole-hero-text' }, status, free, h('div', { class: 'mole-actions' }, live, refresh)),
      h('div', { class: 'mole-gauges' }, ...Object.values(rings)));

    const cpu = tile('cpu', t('mole.cpu'), sparkEl('cpu'), h('div', { class: 'mole-cores' }));
    const mem = tile('mem', t('mole.memory'), sparkEl('mem'), h('div', { class: 'mole-stack' }), h('div', { class: 'mole-legend' }));
    const net = tile('net', t('mole.network'), sparkEl('down'), sparkEl('up'), h('div', { class: 'mole-legend' }));
    const disk = tile('disk', t('mole.disk'), h('div', { class: 'mole-stack' }));
    const heat = tile('heat', t('mole.sensors'), h('div', { class: 'mole-facts' }));
    const tops = ['cpu', 'memory'].map((k) => h('div', { class: 'mole-top ' + k }, h('div', { class: 'mole-tile-head' }, h('span', { text: t('mole.top.' + k) })), h('div', { class: 'mole-top-rows' })));
    const tiles = h('div', { class: 'mole-tiles' }, cpu, mem, net, disk, heat, h('div', { class: 'mole-tops' }, ...tops));

    const kinds = ['clean', 'purge', 'installers'];
    const freeBar = h('div', { class: 'mole-stack big' });
    const freeLegend = h('div', { class: 'mole-legend' });
    const freeBox = h('div', { class: 'mole-free' }, h('div', { class: 'mole-tile-head' }, h('span', { text: t('mole.free.title') }), h('b')), freeBar, freeLegend);

    const stack = (el, parts, whole) => el.replaceChildren(...parts.map(([cls, n]) => {
      const seg = h('span', { class: cls });
      seg.style.width = (whole > 0 ? (n / whole) * 100 : 0) + '%';
      return seg;
    }));
    const legend = (el, parts) => el.replaceChildren(...parts.map(([cls, text]) => h('span', { class: cls }, h('i'), text)));
    const put = (el, html) => { el.hidden = !html.includes('<path'); if (el.innerHTML !== html) el.innerHTML = html; };
    const svg = (body) => '<svg viewBox="0 0 120 36" preserveAspectRatio="none">' + body + '</svg>';

    // While the page is open the health is read again each time a new reading has come in.
    let asked = st().at;
    let askedAt = Date.now();
    reg(() => {
      const s = st();
      if (busy('status') || document.visibilityState !== 'visible') return;
      if (s.at !== asked || Date.now() - askedAt > 5000) { asked = s.at; askedAt = Date.now(); act('mole_status'); }
    });

    reg(() => {
      const s = st();
      remember(s);
      const n = s.score;
      score.className = 'mole-score-ring ' + (n == null ? '' : word(n));
      score.style.setProperty('--v', n || 0);
      score.querySelector('b').textContent = n != null ? String(n) : '--';
      status.textContent = n == null ? t('mole.health.reading') : t('mole.health.' + word(n));
      const lists = kinds.map((k) => mole()[k] || []);
      const can = lists.reduce((a, l) => a + sum(l), 0);
      free.textContent = can ? t('mole.health.can_free', { size: size(can) }) : '';
      refresh.disabled = busy('status');
      live.classList.toggle('on', hist.cpu.length > 1);
      rings.cpu.set(s.cpu); rings.memory.set(s.memory); rings.disk.set(s.disk);
      rings.gpu.hidden = s.gpu == null; rings.gpu.set(s.gpu);

      cpu.value.textContent = Math.round(s.cpu || 0) + '%';
      const cores = s.cores || [];
      const slow = s.kinds ? s.kinds[0] : 0;
      cpu.sub.textContent = s.kinds ? t('mole.cores', { slow: s.kinds[0], fast: s.kinds[1] }) : cores.length ? t('mole.cores.n', { n: cores.length }) : '';
      put(cpu.querySelector('.mole-spark'), svg(spark(hist.cpu, 100)));
      const coreBox = cpu.querySelector('.mole-cores');
      if (coreBox.children.length !== cores.length) coreBox.replaceChildren(...cores.map((_, i) => h('span', { class: i < slow ? 'e' : 'p' }, h('i'))));
      cores.forEach((v, i) => { coreBox.children[i].firstChild.style.height = clamp(v, 2, 100) + '%'; });

      mem.value.textContent = s.memory_total ? size(s.memory_used) + ' / ' + size(s.memory_total) : '';
      mem.sub.textContent = s.pressure ? t('mole.pressure.' + s.pressure) + (s.swap_used ? ' / ' + t('mole.swap', { used: size(s.swap_used), total: size(s.swap_total) }) : '') : '';
      mem.sub.dataset.tone = s.pressure || '';
      put(mem.querySelector('.mole-spark'), svg(spark(hist.memory, 100)));
      const parts = [['app', s.memory_app], ['wired', s.memory_wired], ['compressed', s.memory_compressed]];
      stack(mem.querySelector('.mole-stack'), parts.map(([k, v]) => [k, v || 0]), s.memory_total);
      legend(mem.querySelector('.mole-legend'), parts.map(([k, v]) => [k, t('mole.mem.' + k) + ' ' + size(v || 0)]));

      net.value.textContent = rate(s.net_down || 0);
      net.sub.textContent = t('mole.up') + ' ' + rate(s.net_up || 0);
      const top = Math.max(1024, ...hist.down, ...hist.up);
      const [down, up] = net.querySelectorAll('.mole-spark');
      put(down, svg(spark(hist.down, top)));
      put(up, svg(spark(hist.up, top)));
      legend(net.querySelector('.mole-legend'), [['down', t('mole.down') + ' ' + rate(s.net_down || 0)], ['up', t('mole.up') + ' ' + rate(s.net_up || 0)]]);

      disk.value.textContent = Math.round(s.disk || 0) + '%';
      disk.sub.textContent = s.disk_total ? size(s.disk_used) + ' / ' + size(s.disk_total) + ' / ' + t('mole.disk.free', { size: size(s.disk_total - s.disk_used) }) : '';
      const sizes = kinds.map((k) => sum(mole()[k] || []));
      const other = Math.max(0, (s.disk_used || 0) - sizes.reduce((a, b) => a + b, 0));
      stack(disk.querySelector('.mole-stack'), [['used', other], ...kinds.map((k, i) => [k, sizes[i]])], s.disk_total);

      heat.value.textContent = s.celsius != null ? Math.round(s.celsius) + '\u00b0C' : (s.thermal ? t('mole.thermal.' + s.thermal) : '');
      heat.sub.textContent = s.celsius != null && s.thermal ? t('mole.thermal.' + s.thermal) : '';
      heat.sub.dataset.tone = s.thermal || '';
      const facts = [];
      (s.fans || []).forEach((f, i) => facts.push([t('mole.fan', { n: i + 1 }), Math.round(f.rpm) + ' rpm', f.max ? f.rpm / f.max : 0]));
      if (s.battery) facts.push([t('mole.battery'), s.battery.health + '% / ' + t('mole.battery.cycles', { n: s.battery.cycles }), s.battery.health / 100]);
      heat.querySelector('.mole-facts').replaceChildren(...facts.map(([k, v, share]) => {
        const fill = h('span');
        fill.style.width = clamp(share * 100, 0, 100) + '%';
        return h('div', { class: 'mole-fact' }, h('div', { class: 'mole-meter-label' }, h('span', { text: k }), h('b', { text: v })), h('div', { class: 'mole-bar' }, fill));
      }));
      heat.hidden = s.celsius == null && !facts.length && !s.thermal;

      tops.forEach((box, n) => {
        const list = (n ? s.top_memory : s.top_cpu) || [];
        const top = Math.max(1e-9, ...list.map((p) => (n ? p.memory : p.cpu)));
        box.querySelector('.mole-top-rows').replaceChildren(...(list.length ? list.map((p) => {
          const v = n ? p.memory : p.cpu;
          const fill = h('span');
          fill.style.width = clamp((v / top) * 100, 2, 100) + '%';
          return h('div', { class: 'mole-fact', title: p.name + ' (' + p.pid + ')' },
            h('div', { class: 'mole-meter-label' }, h('span', { class: 'mole-top-name', text: p.name }), h('b', { text: n ? size(v) : v.toFixed(1) + '%' })),
            h('div', { class: 'mole-bar' }, fill));
        }) : [h('div', { class: 'mole-tile-sub', text: t('mole.top.none') })]));
      });

      freeBox.querySelector('b').textContent = size(can);
      freeBox.hidden = !can;
      stack(freeBar, kinds.map((k, i) => [k, sizes[i]]), can);
      legend(freeLegend, kinds.map((k, i) => [k, t('mole.sec.' + k) + ' ' + size(sizes[i])]));
    });
    return [sectionTitle(t('mole.sec.health')), card(hero, tiles, freeBox)];
  }

  /**
   * A list whose rows are chosen with a checkbox and removed together. `detail` is the second line of a row,
   * `lead` an element before its name, `open` makes a folder row open; `head` goes above the toolbar.
   */
  function pickList({ kind, read, detail, lead, open, head, filter, tint, scan, actions }) {
    const key = scan || kind;
    const working = () => busy(actions ? 'docker_act' : 'remove');
    const chosen = new Set();
    const items = () => read() || [];
    const all = h('input', { type: 'checkbox', 'aria-label': t('mole.pick.all') });
    const count = h('span', { class: 'mole-count' });
    const search = filter ? h('input', { type: 'search', class: 'text-field mole-search', placeholder: t('mole.search'), 'aria-label': t('mole.search'), spellcheck: 'false', autocomplete: 'off' }) : null;
    const remove = actions ? null : button(t('mole.' + kind + '.remove'), () => {
      const picked = items().filter((i) => chosen.has(i.path));
      confirmDialog({ title: t('mole.confirm', { n: picked.length, size: size(sum(picked)) }), body: t('mole.' + kind + '.body'), confirm: t('mole.' + kind + '.remove') },
        () => act('mole_remove:' + kind + '\n' + picked.map((i) => i.path).join('\n')));
    }, { icon: 'trash', small: true, danger: true });
    // Docker's lists do things to their rows besides removing them, each its own button.
    const verbs = (actions || []).map(({ verb, label, danger }) => button(t('mole.docker.' + label), () => {
      const picked = items().filter((i) => chosen.has(i.path));
      const go = () => act('mole_docker:' + verb + '\n' + picked.map((i) => i.path).join('\n'));
      if (!danger) { go(); return; }
      confirmDialog({ title: t('mole.confirm', { n: picked.length, size: size(sum(picked)) }), body: t('mole.docker.' + verb + '.body'), confirm: t('mole.docker.' + label) }, go);
    }, { small: true, danger }));
    const refresh = button('', () => act('mole_scan:' + key), { icon: 'refresh', small: true, aria: t('mole.refresh') });
    refresh.title = t('mole.refresh');
    const box = h('div', { class: 'mole-list', 'data-kind': kind });
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
      count.textContent = busy(key) || !read() ? t('mole.scanning')
        : picked.length ? t('mole.picked', { n: picked.length, size: size(sum(picked)) })
        : t('mole.total', { n: list.length, size: size(sum(list)) });
      if (remove) remove.disabled = !picked.length || working();
      for (const b of verbs) b.disabled = !picked.length || working();
      refresh.disabled = busy(key);
    };
    const draw = () => {
      if (!read()) { box.replaceChildren(scanning()); return; }
      const shown = visible();
      if (!shown.length) { box.replaceChildren(busy(key) ? scanning() : h('div', { class: 'empty', text: t('mole.' + kind + '.empty') })); return; }
      const top = Math.max(1, ...shown.map((i) => i.size));
      box.replaceChildren(...shown.map((i, k) => {
        const pick = h('input', { type: 'checkbox', 'aria-label': i.name });
        pick.checked = chosen.has(i.path);
        pick.addEventListener('change', () => { if (pick.checked) chosen.add(i.path); else chosen.delete(i.path); paint(); });
        const fill = h('span');
        fill.style.width = Math.max(1, (i.size / top) * 100) + '%';
        if (tint) fill.style.background = tint(k);
        const go = open && i.dir ? button('', (e) => { e.preventDefault(); open(i); }, { icon: 'folder', small: true, aria: t('mole.analyze.open', { name: i.name }) }) : null;
        const el = h('label', { class: 'mole-item' + (go ? ' opens' : ''), title: tilde(i.path), 'data-path': i.path }, pick, lead ? lead(i) : null,
          h('span', { class: 'mole-item-text' }, h('span', { class: 'mole-item-name', text: i.name }), h('span', { class: 'mole-item-note', text: detail(i) }),
            h('span', { class: 'mole-bar' }, fill)),
          h('span', { class: 'mole-item-size', text: size(i.size) }), go);
        // A folder opens wherever its row is clicked; only the checkbox chooses it.
        if (go) el.addEventListener('click', (e) => { if (e.target !== pick && !go.contains(e.target)) { e.preventDefault(); open(i); } });
        return el;
      }));
    };
    all.addEventListener('change', () => {
      for (const i of visible()) { if (all.checked) chosen.add(i.path); else chosen.delete(i.path); }
      draw(); paint();
    });
    if (search) search.addEventListener('input', () => { draw(); paint(); });
    let last = null;
    reg(() => {
      const stamp = JSON.stringify([read(), busy(key), lang]);
      if (stamp !== last) { last = stamp; draw(); }
      paint();
    });
    return [sectionTitle(t('mole.sec.' + kind)), card(row(t('mole.' + kind + '.title'), t('mole.' + kind + '.desc'), null), head,
      h('div', { class: 'mole-toolbar' }, h('label', { class: 'mole-all' }, all, count), search, ...verbs, remove, refresh), box)];
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

  const PALETTE = ['#5e5ce6', '#0a84ff', '#64d2ff', '#30d158', '#a3d93b', '#ffd60a', '#ff9f0a', '#ff6b35', '#ff375f', '#bf5af2', '#ac8e68'];
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
      wrap.querySelectorAll('.mole-legend-item').forEach((el) => el.classList.toggle('lit', el.dataset.k === k));
      wrap.querySelectorAll('.mole-item.lit').forEach((r) => r.classList.remove('lit'));
      const row = x && x.entry && rowOf(x.entry.path);
      if (row) row.classList.add('lit');
    };
    const keyed = (e) => e.target.closest('.mole-slice, .mole-slice-key');
    wrap.addEventListener('mouseover', (e) => { const el = keyed(e); if (el) light(el.dataset.k); });
    wrap.addEventListener('mouseout', (e) => { if (keyed(e)) light(null); });
    wrap.addEventListener('click', (e) => {
      const el = keyed(e);
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
      cut = a && a.entries && !busy('analyze') ? slices(a) : [];
      const stamp = JSON.stringify([cut.map((x) => [x.name, x.size]), a && a.path, a && a.total, lang]);
      if (stamp === last) return;
      last = stamp;
      wrap.hidden = !cut.length;
      if (!cut.length) { wrap.replaceChildren(); return; }
      let at = 0;
      const paths = cut.map((x, k) => {
        const d = arc(at, at + x.share);
        at += x.share;
        const fill = x.entry ? PALETTE[k % PALETTE.length] : 'var(--text-2)';
        return '<path class="mole-slice' + (x.entry && !x.entry.dir ? ' file' : '') + '" data-k="' + k + '" d="' + d + '" style="fill:' + fill + '"></path>';
      }).join('');
      const top = cut.filter((x) => x.entry).slice(0, 3).map((x) => x.name + ' ' + size(x.size)).join(', ');
      wrap.innerHTML = '<svg viewBox="0 0 200 200" role="img" aria-label="'
        + esc(t('mole.analyze.chart', { path: tilde(a.path), total: size(a.total), top })) + '">' + paths
        + '<foreignObject x="46" y="70" width="108" height="60"><div xmlns="http://www.w3.org/1999/xhtml" class="mole-chart-mid" aria-hidden="true">'
        + '<span class="mole-chart-name"></span><span class="mole-chart-size"></span></div></foreignObject></svg>'
        + '<div class="mole-chart-legend" aria-hidden="true">' + cut.map((x, k) => '<span class="mole-legend-item mole-slice-key" data-k="' + k + '"><i style="background:'
          + (x.entry ? PALETTE[k % PALETTE.length] : 'var(--text-2)') + '"></i>' + esc(x.name) + '<em>' + size(x.size) + '</em></span>').join('') + '</div>';
      light(null);
    });
    return wrap;
  }

  const DOCKER_PARTS = ['Images', 'Containers', 'Local Volumes', 'Build Cache', 'logs'];
  const dockerCls = (k) => 'd-' + k.split(' ').pop().toLowerCase();

  /** What Docker takes, with the clean-ups that work on all of it at once. */
  function dockerCard() {
    const d = () => mole().docker;
    const note = h('div', { class: 'mole-docker-note' });
    const bar = h('div', { class: 'mole-stack big' });
    const legend = h('div', { class: 'mole-legend' });
    const total = h('b');
    const reclaim = h('span', { class: 'mole-tile-sub' });
    const working = () => busy('docker_act') || busy('docker');
    const ask = (label, onYes) => confirmDialog({ title: t('mole.docker.' + label), body: t('mole.docker.' + label + '.body'), confirm: t('mole.docker.' + label) }, onYes);
    const logs = button(t('mole.docker.clear_logs'), () => ask('clear_logs', () => act('mole_docker:logs\n' + d().containers.map((c) => c.path).join('\n'))), { small: true });
    const unused = button(t('mole.docker.unused'), () => ask('unused', () => act('mole_docker_prune:unused')), { icon: 'trash', small: true, danger: true });
    const cache = button(t('mole.docker.cache'), () => ask('cache', () => act('mole_docker_prune:cache')), { icon: 'trash', small: true, danger: true });
    const refresh = button('', () => act('mole_scan:docker'), { icon: 'refresh', small: true, aria: t('mole.refresh') });
    const body = h('div', { class: 'mole-free' }, h('div', { class: 'mole-tile-head' }, reclaim, total), bar, legend);
    const tools = h('div', { class: 'mole-toolbar' }, h('span', { class: 'mole-all' }), logs, unused, cache, refresh);
    reg(() => {
      const v = d();
      const ok = !!(v && v.ok);
      body.hidden = !ok;
      for (const b of [logs, unused, cache]) { b.hidden = !ok; b.disabled = working(); }
      refresh.disabled = busy('docker');
      logs.disabled = working() || !ok || !v.containers.length;
      note.textContent = !v || busy('docker') ? t('mole.scanning') : v.missing ? t('mole.docker.missing') : v.down ? t('mole.docker.down')
        : v.disk ? t('mole.docker.disk', { size: size(v.disk) }) : '';
      if (!ok) return;
      const parts = DOCKER_PARTS.map((k) => {
        const row = k === 'logs' ? { size: v.logs, reclaimable: v.logs } : (v.totals || []).find((x) => x.type === k) || { size: 0, reclaimable: 0 };
        return [k, row];
      });
      const whole = parts.reduce((a, [, r]) => a + r.size, 0);
      total.textContent = size(whole);
      reclaim.textContent = t('mole.docker.reclaim', { size: size(parts.reduce((a, [, r]) => a + r.reclaimable, 0)) });
      bar.replaceChildren(...parts.map(([k, r]) => { const seg = h('span', { class: dockerCls(k) }); seg.style.width = (whole ? (r.size / whole) * 100 : 0) + '%'; return seg; }));
      legend.replaceChildren(...parts.map(([k, r]) => h('span', { class: dockerCls(k) }, h('i'), t('mole.docker.t.' + k) + ' ' + size(r.size))));
    });
    return [sectionTitle(t('mole.sec.docker')), card(row(t('mole.docker.title'), t('mole.docker.desc'), null), h('div', { class: 'mole-free' }, note), body, tools)];
  }

  function dockerLists() {
    const list = (k) => () => (mole().docker && mole().docker.ok ? mole().docker[k] : null);
    const used = (i) => (i.used ? t('mole.docker.used', { n: i.used }) : t('mole.docker.unused.tag'));
    const wrap = h('div', { class: 'mole-docker' },
      ...pickList({ kind: 'docker_containers', scan: 'docker', read: list('containers'), filter: true,
        lead: (i) => h('span', { class: 'mole-dot ' + i.state, title: i.state }),
        detail: (i) => [i.note, i.status, i.logs ? t('mole.docker.logs', { size: size(i.logs) }) : ''].filter(Boolean).join(' / '),
        actions: [{ verb: 'start', label: 'start' }, { verb: 'stop', label: 'stop' }, { verb: 'restart', label: 'restart' },
          { verb: 'logs', label: 'clean', danger: true }, { verb: 'rm', label: 'remove', danger: true }] }),
      ...pickList({ kind: 'docker_images', scan: 'docker', read: list('images'), filter: true,
        detail: (i) => [i.note, used(i)].join(' / '), actions: [{ verb: 'rmi', label: 'remove', danger: true }] }),
      ...pickList({ kind: 'docker_volumes', scan: 'docker', read: list('volumes'), filter: true,
        detail: (i) => [i.anonymous ? t('mole.docker.anonymous') : '', used(i)].filter(Boolean).join(' / '), actions: [{ verb: 'volume_rm', label: 'remove', danger: true }] }),
      ...pickList({ kind: 'docker_networks', scan: 'docker', read: list('networks'),
        detail: (i) => i.note, actions: [{ verb: 'network_rm', label: 'remove', danger: true }] }));
    reg(() => { wrap.hidden = !(mole().docker && mole().docker.ok); });
    return wrap;
  }

  function analyzeHead() {
    const where = h('nav', { class: 'mole-path', 'aria-label': t('mole.sec.analyze') });
    const up = button(t('mole.analyze.up'), () => act('mole_analyze:' + (mole().analyze.path.replace(/\/[^/]+\/?$/, '') || '/')), { icon: 'chevron.up', small: true });
    const home = button(t('mole.analyze.home'), () => act('mole_analyze:' + mole().home), { small: true });
    reg(() => {
      const a = mole().analyze;
      const stamp = a ? a.path + '|' + a.total + '|' + busy('analyze') + '|' + lang : '';
      if (where.dataset.stamp !== stamp) {
        where.dataset.stamp = stamp;
        // Each folder of the path is a step back up to it, the home folder counted as one.
        const home = mole().home;
        const inHome = a && home && (a.path === home || a.path.startsWith(home + '/'));
        const rest = a ? (inHome ? a.path.slice(home.length) : a.path).split('/').filter(Boolean) : [];
        const steps = a ? [[inHome ? home : '/', inHome ? '~' : '/'], ...rest.map((n, k) => [(inHome ? home : '') + '/' + rest.slice(0, k + 1).join('/'), n])] : [];
        where.replaceChildren(...steps.map(([path, name], k) => k === steps.length - 1
          ? h('b', { text: name })
          : h('button', { class: 'mole-crumb', type: 'button', text: name, onclick: () => act('mole_analyze:' + path) })),
          a ? h('span', { class: 'mole-path-size', text: a.total != null ? size(a.total) : t('mole.scanning') }) : null);
      }
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
        if (r.docker) toast(t(r.failed ? 'mole.docker.done.partial' : 'mole.docker.done', { n: r.removed, failed: r.failed }));
        else toast(t(r.failed ? 'mole.result.partial' : 'mole.result', { n: r.removed, size: size(r.freed), failed: r.failed }));
      });
      act('mole_open');
      return [
        ...healthCard(),
        ...pickList({ kind: 'clean', read: () => mole().clean, detail: (i) => t('mole.group.' + i.note) + ' / ' + tilde(i.path) }),
        ...pickList({ kind: 'purge', read: () => mole().purge, detail: (i) => [i.note, age(i)].filter(Boolean).join(' / ') }),
        ...pickList({ kind: 'installers', read: () => mole().installers, detail: (i) => tilde(i.path) }),
        ...pickList({ kind: 'apps', read: () => mole().apps, filter: true, lead: (i) => iconImg(i.note),
          detail: (i) => (i.extra ? i.note + ' / ' + t('mole.apps.extra', { n: i.extra }) : i.note) }),
        ...dockerCard(),
        dockerLists(),
        ...pickList({ kind: 'analyze', read: () => mole().analyze && mole().analyze.entries, head: analyzeHead(), detail: () => '',
          open: (i) => act('mole_analyze:' + i.path), tint: (k) => (k < MAX_SLICES - 1 ? PALETTE[k % PALETTE.length] : 'var(--text-2)') }),
      ];
    },
    selftest({ expect, go, sent, S }) {
      const home = S.mole.home;
      S.mole.status = { score: 91, cpu: 13.6, memory: 81.2, memory_used: 1, memory_total: 2, swap_used: 3858169856, swap_total: 5368709120, disk: 48.2, disk_used: 238313164359, disk_total: 494384795648,
        cores: [10, 20, 90, 40], kinds: [2, 2], top_cpu: [{ pid: 7, name: 'Xcode', cpu: 182.4, memory: 9 }], top_memory: [], memory_app: 1, memory_wired: 0, memory_compressed: 0, pressure: 'warning', net_down: 2048, net_up: 0, gpu: null, celsius: null, thermal: 'nominal', fans: [], battery: null };
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
      expect(document.querySelectorAll('.mole-cores .e').length === 2 && document.querySelectorAll('.mole-cores .p').length === 2, 'mole: the cores are drawn by kind');
      expect(document.querySelector('.mole-top.cpu').textContent.includes('Xcode182.4%') && document.querySelector('.mole-top.memory').textContent.includes(t('mole.top.none')),
        'mole: the busiest processes are listed, and an empty list says so');
      expect(getComputedStyle(document.querySelector('.mole-gauge.gpu')).display === 'none', 'mole: a Mac that says nothing of its graphics has no ring for it');
      sent.length = 0;
      section('analyze').querySelector('.mole-item.opens .mole-item-name').click();
      expect(sent.some((m) => m.name === 'mole_analyze:' + home + '/Library'), 'mole: a folder row opens where it is clicked: ' + JSON.stringify(sent));
      expect(!section('analyze').querySelector('.mole-item input:checked'), 'mole: opening a folder does not choose it');
      S.mole.analyze = { path: home + '/Library/Caches', total: null, entries: null }; runSyncs();
      const crumbs = [...section('analyze').querySelectorAll('.mole-crumb')];
      expect(crumbs.length === 2 && section('analyze').textContent.includes(t('mole.scanning')), 'mole: a folder being read shows its path and that it is scanning');
      sent.length = 0;
      crumbs[1].click();
      expect(sent.some((m) => m.name === 'mole_analyze:' + home + '/Library'), 'mole: a step of the path goes back up to it: ' + JSON.stringify(sent));
      S.mole.analyze = { path: home, total: 300, entries: [{ path: home + '/Library', name: 'Library', note: '', size: 200, dir: true, age: 0, extra: 0 }] }; runSyncs();
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
      S.mole.docker = { missing: true }; runSyncs();
      expect(document.body.textContent.includes(t('mole.docker.missing')) && document.querySelector('.mole-docker').hidden, 'mole: without Docker the page says so and lists nothing');
      const cid = 'a'.repeat(64);
      S.mole.docker = { ok: true, logs: 2048, disk: 5e9, totals: [{ type: 'Images', size: 3e9, reclaimable: 1e9, count: 2 }, { type: 'Build Cache', size: 2e9, reclaimable: 2e9, count: 9 }],
        containers: [{ path: cid, name: 'web', note: 'nginx', status: 'Up 2 minutes', state: 'running', logs: 2048, size: 4096, dir: false }],
        images: [{ path: 'sha256:1', name: 'nginx:latest', note: '2 days ago', used: 1, size: 100, dir: false }],
        volumes: [{ path: 'data', name: 'data', anonymous: false, used: 0, size: 10, note: '', dir: false }], networks: [] };
      runSyncs();
      const dsec = document.querySelector('.mole-list[data-kind=docker_containers]').closest('.card');
      expect(!document.querySelector('.mole-docker').hidden && dsec.querySelector('.mole-dot.running') && dsec.textContent.includes(t('mole.docker.logs', { size: size(2048) })),
        'mole: a running container is listed with its logs');
      expect(section('docker').textContent.includes(t('mole.docker.t.Build Cache') + ' ' + size(2e9)), 'mole: the parts of Docker are added up');
      dsec.querySelector('.mole-all input').click();
      sent.length = 0;
      btn(dsec, t('mole.docker.stop')).click();
      expect(sent.some((m) => m.name === 'mole_docker:stop\n' + cid), 'mole: stopping needs no question: ' + JSON.stringify(sent));
      sent.length = 0;
      btn(dsec, t('mole.docker.remove')).click();
      expect(!sent.length, 'mole: removing a container asks first');
      document.querySelector('#modal .btn.primary').click();
      expect(sent.some((m) => m.name === 'mole_docker:rm\n' + cid), 'mole: the chosen container is removed: ' + JSON.stringify(sent));
      sent.length = 0;
      btn(section('docker'), t('mole.docker.clear_logs')).click();
      document.querySelector('#modal .btn.primary').click();
      expect(sent.some((m) => m.name === 'mole_docker:logs\n' + cid), 'mole: clear all logs takes every container: ' + JSON.stringify(sent));
      sent.length = 0;
      btn(section('docker'), t('mole.docker.cache')).click();
      document.querySelector('#modal .btn.primary').click();
      expect(sent.some((m) => m.name === 'mole_docker_prune:cache'), 'mole: the build cache is cleared after asking');
      S.mole.busy = ['docker_act']; runSyncs();
      expect(btn(dsec, t('mole.docker.stop')).disabled, 'mole: no second Docker action while one runs');
      S.mole.busy = []; S.mole.docker = null;
      S.mole.status = null; S.mole.clean = null; S.mole.purge = null; S.mole.apps = null; S.mole.analyze = null; runSyncs();
    },
  });
})();
