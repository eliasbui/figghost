'use strict';
// Figghost settings page. The app talks to it through window.fig.send(message) and window.__fig.recv(message).

// ---------------------------------------------------------------- utilities

const $ = (selector, root = document) => root.querySelector(selector);
const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));

function h(tag, attrs, ...kids) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs || {})) {
    if (value === false || value == null) continue;
    if (key === 'class') el.className = value;
    else if (key === 'text') el.textContent = value;
    else if (key === 'html') el.innerHTML = value;
    else if (key.startsWith('on')) el.addEventListener(key.slice(2), value);
    else el.setAttribute(key, value === true ? '' : value);
  }
  for (const kid of kids.flat()) {
    if (kid == null || kid === false) continue;
    el.append(kid.nodeType ? kid : document.createTextNode(String(kid)));
  }
  return el;
}

function icon(name, cls) {
  const el = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  el.setAttribute('viewBox', '0 0 24 24');
  el.setAttribute('aria-hidden', 'true');
  if (cls) el.setAttribute('class', cls);
  el.innerHTML = ICONS[name] || '';
  return el;
}

const ICONS = {
  general: '<path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1"/><circle cx="15" cy="6" r="2"/><circle cx="9" cy="12" r="2"/><circle cx="17" cy="18" r="2"/>',
  scroll: '<rect x="6" y="3" width="12" height="18" rx="6"/><path d="M12 7v4"/>',
  clipboard: '<rect x="5" y="4" width="14" height="17" rx="3"/><path d="M9 4.5h6M9 10h6M9 14h4"/>',
  notch: '<rect x="3" y="4" width="18" height="13" rx="3"/><path d="M9 4v2.2a1 1 0 0 0 1 1h4a1 1 0 0 0 1-1V4"/><path d="M8 21h8"/>',
  tiling: '<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M12 4v16M12 12h9"/>',
  about: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 7.6v.01"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  close: '<path d="M6 6l12 12M18 6L6 18"/>',
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  refresh: '<path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7"/>',
  trash: '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3"/>',
  power: '<path d="M12 3v9M6.3 6.8a8 8 0 1 0 11.4 0"/>',
};

const GHOST = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2.5c-4.2 0-7.5 3.2-7.5 7.3V20c0 .8.9 1.2 1.5.7l1.6-1.4 1.6 1.4c.5.4 1.2.4 1.7 0l1.1-1 1.1 1c.5.4 1.2.4 1.7 0l1.6-1.4 1.6 1.4c.6.5 1.5.1 1.5-.7V9.8c0-4.1-3.3-7.3-7.5-7.3z"/><circle cx="9.2" cy="10.2" r="1.5" fill="#5b4bdc"/><circle cx="14.8" cy="10.2" r="1.5" fill="#5b4bdc"/></svg>';

function throttleBy(key, ms, fn) {
  const slots = (throttleBy.slots = throttleBy.slots || new Map());
  const slot = slots.get(key) || { last: 0, timer: 0, args: null };
  slots.set(key, slot);
  slot.args = [fn];
  const flush = () => { slot.timer = 0; slot.last = performance.now(); const [f] = slot.args; f(); };
  const wait = slot.last + ms - performance.now();
  if (wait <= 0) { clearTimeout(slot.timer); flush(); } else if (!slot.timer) slot.timer = setTimeout(flush, wait);
}

// ---------------------------------------------------------------- language

const I18N = {
  en: {
    'nav.general': 'General', 'nav.scroll': 'Scrolling', 'nav.clipboard': 'Clipboard', 'nav.notch': 'Notch', 'nav.tiling': 'Window snapping', 'nav.about': 'About',
    'page.general': 'General', 'page.general.sub': 'Startup, language and permissions.',
    'page.scroll': 'Scrolling', 'page.scroll.sub': 'Smooth, reversible mouse-wheel scrolling. Trackpads and the Magic Mouse are never touched.',
    'page.clipboard': 'Clipboard', 'page.clipboard.sub': 'A history of everything you copy, with instant search, pinning and keyboard shortcuts.',
    'page.notch': 'Notch', 'page.notch.sub': 'A panel that grows out of the top edge of your screen: clipboard, file shelf and media.',
    'page.tiling': 'Window snapping', 'page.tiling.sub': 'Place the focused window on a part of the screen with a shortcut.',
    'page.about': 'About', 'page.about.sub': 'Mos, Maccy and OneNotch ideas in one small app.',

    'hero.tagline': 'Smooth scrolling, clipboard history and a notch toolbox in one quiet menu bar app.',
    'chip.version': 'Version {v}', 'chip.access.ok': 'Accessibility on', 'chip.access.no': 'Accessibility needed',
    'sec.startup': 'Startup', 'sec.language': 'Language', 'sec.permissions': 'Permissions', 'sec.data': 'Data',
    'login.title': 'Launch at login', 'login.desc': 'Start Figghost when you log in. The window stays closed; use the menu bar icon.',
    'login.approval': 'Allow Figghost in System Settings, under Login Items, to finish.', 'login.unavailable': 'Available when Figghost runs as an app from a folder such as Applications.',
    'login.open': 'Open Login Items',
    'showlaunch.title': 'Open this window at start', 'showlaunch.desc': 'Show settings each time you open the app yourself. Never at login.',
    'lang.title': 'Language', 'lang.desc': 'This window changes at once. Menus and the notch change after a restart.',
    'lang.auto': 'Automatic', 'lang.en': 'English', 'lang.vi': 'Tiếng Việt',
    'lang.restart': 'Restart Figghost to use the new language in menus and the notch.', 'lang.restart.btn': 'Restart now',
    'access.title': 'Accessibility', 'access.desc': 'Needed for smooth scrolling, pasting into other apps, window snapping and the volume indicator.',
    'access.granted': 'Granted', 'access.missing': 'Not granted', 'access.open': 'Open System Settings',
    'data.title': 'Settings folder', 'data.desc': 'Where your settings, clipboard history and shelf are stored. Everything stays on this Mac.', 'data.open': 'Open folder',

    'sec.wheel': 'Mouse wheel', 'sec.feel': 'Feel', 'sec.try': 'Try it', 'sec.mods': 'Modifier keys', 'sec.apps': 'Apps',
    'smooth.title': 'Smooth scrolling', 'smooth.desc': 'Turns each wheel notch into a gliding motion.',
    'reverse.title': 'Reverse wheel direction', 'reverse.desc': 'Only mouse wheels are reversed. The trackpad keeps its natural direction.',
    'step.title': 'Step', 'step.desc': 'Distance of one wheel notch', 'speed.title': 'Speed', 'speed.desc': 'Multiplier for every scroll', 'duration.title': 'Glide', 'duration.desc': 'How long a glide takes to settle',
    'preset.precise': 'Precise', 'preset.balanced': 'Balanced', 'preset.fluid': 'Fluid',
    'graph.distance': 'Distance', 'graph.velocity': 'Speed', 'graph.per': 'One notch moves {px} px; 98% of it is done in {ms} ms.',
    'pad.hint': 'Scroll here with your mouse wheel to feel the settings.', 'pad.speed': 'Speed', 'pad.peak': 'Peak', 'pad.frames': 'Frames', 'pad.jump': 'Largest jump', 'pad.line': 'line {n}',
    'shift.title': 'Shift scrolls sideways', 'shift.desc': 'Hold Shift and turn the wheel to scroll horizontally, smoothly.',
    'option.title': 'Option scrolls faster', 'option.desc': 'Hold Option to scroll several times as far. Good for long documents.',
    'boost.title': 'Boost', 'cmd.title': 'Command zooms', 'cmd.desc': 'Hold Command and turn the wheel to zoom in or out. Sends Command + and Command - in browsers, editors and viewers.',
    'apps.desc': 'With Smooth scrolling on, set an app to Off to leave it alone (a block list). With it off, set an app to On and only those apps glide (an allow list).',
    'apps.empty': 'No app rules yet.', 'apps.add': 'Add app', 'apps.smooth': 'Smooth', 'apps.reverse': 'Reverse', 'rule.default': 'Default', 'rule.on': 'On', 'rule.off': 'Off', 'apps.remove': 'Remove',
    'menu.running': 'Running apps', 'menu.disk': 'Choose from disk...', 'menu.none': 'No other apps are open',

    'sec.shortcut': 'Shortcut', 'sec.pasting': 'Pasting', 'sec.history': 'History', 'sec.privacy': 'Privacy', 'sec.keys': 'Keyboard',
    'clip.hotkey.title': 'Open clipboard history', 'clip.hotkey.desc': 'Works everywhere. The popup appears next to the pointer.',
    'paste.auto.title': 'Paste automatically', 'paste.auto.desc': 'Press Enter on an item to paste it into the app you were using. Off copies only.',
    'paste.plain.title': 'Paste without formatting', 'paste.plain.desc': 'Text goes in as plain text. Option + Shift + Enter does the opposite for one paste.',
    'limit.title': 'Items to keep', 'limit.desc': 'Pinned items are never removed.', 'count.title': 'In the history now',
    'clear.title': 'Clear history', 'clear.desc': 'Removes every item that is not pinned.', 'clear.btn': 'Clear',
    'clear.confirm.title': 'Clear the clipboard history?', 'clear.confirm.body': 'Every item that is not pinned is removed. This cannot be undone.', 'cancel': 'Cancel',
    'ignored.title': 'Never record copies from', 'ignored.desc': 'Password managers and items marked as concealed are always skipped.', 'ignored.empty': 'No apps added.', 'ignored.add': 'Add app',
    'key.paste': 'Paste', 'key.pastealt': 'Paste the other way', 'key.pin': 'Pin or unpin', 'key.delete': 'Delete', 'key.close': 'Close', 'key.pinkey': 'Paste a pinned item', 'key.filters': 'Filters',
    'key.filters.val': '@image @link @code @file @pin',

    'notch.panel.title': 'Notch panel', 'notch.panel.desc': 'Move the pointer to the top edge of the screen to open it. Off removes it entirely.',
    'notch.preview': 'Hover the top edge', 'sec.tabs': 'Tabs', 'tab.clipboard': 'Clipboard', 'tab.clipboard.desc': 'The same list as the popup, with search.',
    'tab.shelf': 'Shelf', 'tab.shelf.desc': 'Drop files to keep them handy, and drag them out again. Only references are stored.', 'tab.media': 'Media', 'tab.media.desc': 'Previous, play and next for whatever is playing; track details for Music and Spotify.',
    'sec.volume': 'Volume', 'vol.title': 'Volume indicator in the notch', 'vol.desc': 'Replaces the system volume display. Needs Accessibility. Shift + Option moves in finer steps.',
    'shelf.count': 'Files on the shelf', 'shelf.clear': 'Clear shelf', 'shelf.clear.desc': 'Removes the references only. Your files are not touched.',
    'sec.shelf': 'Shelf',

    'tiling.enable.title': 'Window snapping', 'tiling.enable.desc': 'Move and resize the focused window with a shortcut. Needs Accessibility.',
    'tiling.reset': 'Reset shortcuts', 'tiling.none': 'None', 'tiling.clear': 'Clear shortcut',
    'layout.left': 'Left half', 'layout.right': 'Right half', 'layout.top': 'Top half', 'layout.bottom': 'Bottom half', 'layout.top_left': 'Top left', 'layout.top_right': 'Top right', 'layout.bottom_left': 'Bottom left', 'layout.bottom_right': 'Bottom right',
    'layout.maximize': 'Maximize', 'layout.center': 'Center', 'layout.first_third': 'First third', 'layout.center_third': 'Center third', 'layout.last_third': 'Last third',

    'about.line': 'Smooth scrolling like Mos, clipboard history like Maccy, a notch toolbox like OneNotch.',
    'about.version': 'Version', 'about.privacy': 'Privacy', 'about.privacy.val': 'Everything stays on this Mac. Nothing is sent anywhere.', 'about.quit': 'Quit Figghost',
    'about.quit.desc': 'Stops scrolling, clipboard and notch features until you open the app again.',

    'rec.press': 'Press shortcut', 'rec.esc': 'Esc to cancel',
    'fault.invalid_value': 'That value is not valid.', 'fault.unknown_setting': 'Unknown setting.', 'fault.invalid_app_id': 'That app cannot be used.', 'fault.needs_modifier': 'Add Control, Option, Shift or Command.',
    'fault.invalid_shortcut': 'That key cannot be used.', 'fault.shortcut_required': 'This shortcut cannot be removed.', 'fault.conflict': 'Already used by {name}.', 'fault.taken': 'Another app already uses this shortcut.',
    'fault.accessibility_needed': 'Accessibility access is needed. Allow Figghost in System Settings.', 'fault.login_failed': 'Could not change the login item: {detail}', 'fault.unknown_action': 'Unknown action.', 'fault.unsupported_key': 'That key is not supported.',
    'target.clipboard': 'the clipboard shortcut',
  },
  vi: {
    'nav.general': 'Chung', 'nav.scroll': 'Cuộn chuột', 'nav.clipboard': 'Clipboard', 'nav.notch': 'Notch', 'nav.tiling': 'Chia cửa sổ', 'nav.about': 'Giới thiệu',
    'page.general': 'Chung', 'page.general.sub': 'Khởi động, ngôn ngữ và quyền truy cập.',
    'page.scroll': 'Cuộn chuột', 'page.scroll.sub': 'Cuộn mượt và đảo chiều cho con lăn chuột. Trackpad và Magic Mouse không bị can thiệp.',
    'page.clipboard': 'Clipboard', 'page.clipboard.sub': 'Lịch sử mọi thứ bạn sao chép, tìm kiếm tức thì, ghim và phím tắt.',
    'page.notch': 'Notch', 'page.notch.sub': 'Bảng mọc ra từ mép trên màn hình: clipboard, khay tệp và media.',
    'page.tiling': 'Chia cửa sổ', 'page.tiling.sub': 'Đặt cửa sổ đang dùng vào một vùng của màn hình bằng phím tắt.',
    'page.about': 'Giới thiệu', 'page.about.sub': 'Ý tưởng của Mos, Maccy và OneNotch trong một ứng dụng nhỏ.',

    'hero.tagline': 'Cuộn mượt, lịch sử clipboard và hộp công cụ notch trong một ứng dụng thanh menu gọn nhẹ.',
    'chip.version': 'Phiên bản {v}', 'chip.access.ok': 'Đã có quyền Trợ năng', 'chip.access.no': 'Cần quyền Trợ năng',
    'sec.startup': 'Khởi động', 'sec.language': 'Ngôn ngữ', 'sec.permissions': 'Quyền truy cập', 'sec.data': 'Dữ liệu',
    'login.title': 'Mở khi đăng nhập', 'login.desc': 'Chạy Figghost khi bạn đăng nhập. Cửa sổ này không tự mở; dùng biểu tượng trên thanh menu.',
    'login.approval': 'Hãy cho phép Figghost trong Cài đặt hệ thống, mục Mục đăng nhập, để hoàn tất.', 'login.unavailable': 'Chỉ dùng được khi Figghost chạy như một ứng dụng đặt trong thư mục như Applications.',
    'login.open': 'Mở Mục đăng nhập',
    'showlaunch.title': 'Mở cửa sổ này khi khởi động', 'showlaunch.desc': 'Hiện Cài đặt mỗi khi bạn tự mở ứng dụng. Không bao giờ khi đăng nhập.',
    'lang.title': 'Ngôn ngữ', 'lang.desc': 'Cửa sổ này đổi ngay. Menu và notch đổi sau khi khởi động lại.',
    'lang.auto': 'Tự động', 'lang.en': 'English', 'lang.vi': 'Tiếng Việt',
    'lang.restart': 'Hãy khởi động lại Figghost để dùng ngôn ngữ mới trong menu và notch.', 'lang.restart.btn': 'Khởi động lại',
    'access.title': 'Trợ năng', 'access.desc': 'Cần cho cuộn mượt, dán vào ứng dụng khác, chia cửa sổ và chỉ báo âm lượng.',
    'access.granted': 'Đã cấp', 'access.missing': 'Chưa cấp', 'access.open': 'Mở Cài đặt hệ thống',
    'data.title': 'Thư mục dữ liệu', 'data.desc': 'Nơi lưu cài đặt, lịch sử clipboard và khay tệp. Mọi thứ nằm trên máy Mac này.', 'data.open': 'Mở thư mục',

    'sec.wheel': 'Con lăn chuột', 'sec.feel': 'Cảm giác cuộn', 'sec.try': 'Thử ngay', 'sec.mods': 'Phím bổ trợ', 'sec.apps': 'Ứng dụng',
    'smooth.title': 'Cuộn mượt', 'smooth.desc': 'Biến mỗi nấc lăn thành chuyển động trượt êm.',
    'reverse.title': 'Đảo chiều con lăn', 'reverse.desc': 'Chỉ đảo con lăn của chuột. Trackpad giữ chiều tự nhiên.',
    'step.title': 'Bước cuộn', 'step.desc': 'Quãng đường của một nấc lăn', 'speed.title': 'Tốc độ', 'speed.desc': 'Hệ số nhân cho mọi lần cuộn', 'duration.title': 'Độ trượt', 'duration.desc': 'Thời gian để chuyển động dừng hẳn',
    'preset.precise': 'Chính xác', 'preset.balanced': 'Cân bằng', 'preset.fluid': 'Mượt',
    'graph.distance': 'Quãng đường', 'graph.velocity': 'Tốc độ', 'graph.per': 'Một nấc lăn đi {px} px; 98% hoàn thành sau {ms} ms.',
    'pad.hint': 'Lăn chuột tại đây để cảm nhận các thiết lập.', 'pad.speed': 'Tốc độ', 'pad.peak': 'Cao nhất', 'pad.frames': 'Số khung', 'pad.jump': 'Bước nhảy lớn nhất', 'pad.line': 'dòng {n}',
    'shift.title': 'Shift để cuộn ngang', 'shift.desc': 'Giữ Shift và lăn chuột để cuộn ngang, vẫn mượt.',
    'option.title': 'Option để cuộn nhanh', 'option.desc': 'Giữ Option để cuộn xa gấp nhiều lần. Hợp với tài liệu dài.',
    'boost.title': 'Hệ số', 'cmd.title': 'Command để phóng to', 'cmd.desc': 'Giữ Command và lăn chuột để phóng to hoặc thu nhỏ. Gửi Command + và Command - trong trình duyệt, trình soạn thảo và trình xem ảnh.',
    'apps.desc': 'Khi Cuộn mượt đang bật, đặt một ứng dụng sang Tắt để bỏ qua nó (danh sách chặn). Khi đang tắt, đặt ứng dụng sang Bật thì chỉ các ứng dụng đó được làm mượt (danh sách cho phép).',
    'apps.empty': 'Chưa có quy tắc nào.', 'apps.add': 'Thêm ứng dụng', 'apps.smooth': 'Cuộn mượt', 'apps.reverse': 'Đảo chiều', 'rule.default': 'Mặc định', 'rule.on': 'Bật', 'rule.off': 'Tắt', 'apps.remove': 'Xóa',
    'menu.running': 'Ứng dụng đang chạy', 'menu.disk': 'Chọn từ ổ đĩa...', 'menu.none': 'Không có ứng dụng nào khác đang mở',

    'sec.shortcut': 'Phím tắt', 'sec.pasting': 'Dán', 'sec.history': 'Lịch sử', 'sec.privacy': 'Riêng tư', 'sec.keys': 'Bàn phím',
    'clip.hotkey.title': 'Mở lịch sử clipboard', 'clip.hotkey.desc': 'Dùng ở mọi nơi. Cửa sổ nhỏ hiện cạnh con trỏ.',
    'paste.auto.title': 'Dán tự động', 'paste.auto.desc': 'Nhấn Enter ở một mục để dán vào ứng dụng bạn đang dùng. Tắt thì chỉ sao chép.',
    'paste.plain.title': 'Dán không định dạng', 'paste.plain.desc': 'Văn bản vào ở dạng thuần. Option + Shift + Enter làm ngược lại cho một lần dán.',
    'limit.title': 'Số mục giữ lại', 'limit.desc': 'Mục đã ghim không bao giờ bị xóa.', 'count.title': 'Hiện có trong lịch sử',
    'clear.title': 'Xóa lịch sử', 'clear.desc': 'Xóa mọi mục chưa ghim.', 'clear.btn': 'Xóa',
    'clear.confirm.title': 'Xóa lịch sử clipboard?', 'clear.confirm.body': 'Mọi mục chưa ghim sẽ bị xóa. Không thể hoàn tác.', 'cancel': 'Hủy',
    'ignored.title': 'Không ghi lại nội dung sao chép từ', 'ignored.desc': 'Trình quản lý mật khẩu và mục được đánh dấu ẩn luôn bị bỏ qua.', 'ignored.empty': 'Chưa thêm ứng dụng nào.', 'ignored.add': 'Thêm ứng dụng',
    'key.paste': 'Dán', 'key.pastealt': 'Dán theo kiểu ngược lại', 'key.pin': 'Ghim hoặc bỏ ghim', 'key.delete': 'Xóa', 'key.close': 'Đóng', 'key.pinkey': 'Dán một mục đã ghim', 'key.filters': 'Bộ lọc',
    'key.filters.val': '@image @link @code @file @pin',

    'notch.panel.title': 'Bảng notch', 'notch.panel.desc': 'Đưa con trỏ lên mép trên màn hình để mở. Tắt thì bảng biến mất hoàn toàn.',
    'notch.preview': 'Rê chuột lên mép trên', 'sec.tabs': 'Các tab', 'tab.clipboard': 'Clipboard', 'tab.clipboard.desc': 'Cùng danh sách như cửa sổ nhỏ, có tìm kiếm.',
    'tab.shelf': 'Khay', 'tab.shelf.desc': 'Thả tệp vào để giữ tạm và kéo ra lại. Chỉ lưu đường dẫn tham chiếu.', 'tab.media': 'Media', 'tab.media.desc': 'Trước, phát và sau cho ứng dụng đang phát; thông tin bài hát cho Music và Spotify.',
    'sec.volume': 'Âm lượng', 'vol.title': 'Chỉ báo âm lượng trong notch', 'vol.desc': 'Thay màn hình âm lượng của hệ thống. Cần quyền Trợ năng. Shift + Option để chỉnh từng bước nhỏ.',
    'shelf.count': 'Tệp trên khay', 'shelf.clear': 'Dọn khay', 'shelf.clear.desc': 'Chỉ xóa phần tham chiếu. Tệp của bạn không bị động tới.',
    'sec.shelf': 'Khay tệp',

    'tiling.enable.title': 'Chia cửa sổ', 'tiling.enable.desc': 'Di chuyển và đổi cỡ cửa sổ đang dùng bằng phím tắt. Cần quyền Trợ năng.',
    'tiling.reset': 'Đặt lại phím tắt', 'tiling.none': 'Chưa có', 'tiling.clear': 'Xóa phím tắt',
    'layout.left': 'Nửa trái', 'layout.right': 'Nửa phải', 'layout.top': 'Nửa trên', 'layout.bottom': 'Nửa dưới', 'layout.top_left': 'Góc trên trái', 'layout.top_right': 'Góc trên phải', 'layout.bottom_left': 'Góc dưới trái', 'layout.bottom_right': 'Góc dưới phải',
    'layout.maximize': 'Phóng to tối đa', 'layout.center': 'Giữa màn hình', 'layout.first_third': 'Một phần ba đầu', 'layout.center_third': 'Một phần ba giữa', 'layout.last_third': 'Một phần ba cuối',

    'about.line': 'Cuộn mượt như Mos, lịch sử clipboard như Maccy, hộp công cụ notch như OneNotch.',
    'about.version': 'Phiên bản', 'about.privacy': 'Riêng tư', 'about.privacy.val': 'Mọi thứ nằm trên máy Mac này. Không gửi đi đâu cả.', 'about.quit': 'Thoát Figghost',
    'about.quit.desc': 'Dừng cuộn mượt, clipboard và notch cho đến khi bạn mở lại ứng dụng.',

    'rec.press': 'Nhấn phím tắt', 'rec.esc': 'Esc để hủy',
    'fault.invalid_value': 'Giá trị không hợp lệ.', 'fault.unknown_setting': 'Thiết lập không tồn tại.', 'fault.invalid_app_id': 'Không dùng được ứng dụng này.', 'fault.needs_modifier': 'Thêm Control, Option, Shift hoặc Command.',
    'fault.invalid_shortcut': 'Không dùng được phím này.', 'fault.shortcut_required': 'Không thể xóa phím tắt này.', 'fault.conflict': 'Đã được dùng cho {name}.', 'fault.taken': 'Ứng dụng khác đang dùng phím tắt này.',
    'fault.accessibility_needed': 'Cần quyền Trợ năng. Hãy cho phép Figghost trong Cài đặt hệ thống.', 'fault.login_failed': 'Không đổi được mục đăng nhập: {detail}', 'fault.unknown_action': 'Hành động không tồn tại.', 'fault.unsupported_key': 'Phím này chưa được hỗ trợ.',
    'target.clipboard': 'phím tắt clipboard',
  },
};

let lang = 'en';
const missingKeys = new Set();
function t(key, vars) {
  if (!I18N.en[key]) missingKeys.add(key);
  let text = (I18N[lang] && I18N[lang][key]) || I18N.en[key] || key;
  for (const [name, value] of Object.entries(vars || {})) text = text.replaceAll('{' + name + '}', value);
  return text;
}

function resolveLang(setting) {
  if (setting === 'en' || setting === 'vi') return setting;
  for (const tag of navigator.languages || [navigator.language || 'en']) {
    const primary = String(tag).slice(0, 2).toLowerCase();
    if (I18N[primary]) return primary;
  }
  return 'en';
}

// ---------------------------------------------------------------- state and bridge

let S = null;               // the latest state from the app
const syncs = [];           // functions that bring the visible controls in line with S
const iconCache = {};
const pendingIcons = new Set();
let iconTimer = 0;

function send(message) { window.fig.send(message); }
function get(path) { return path.split('.').reduce((o, k) => (o == null ? undefined : o[k]), S); }
function setLocal(path, value) {
  const keys = path.split('.');
  const last = keys.pop();
  keys.reduce((o, k) => o[k], S)[last] = value;
}
function reg(fn) { syncs.push(fn); fn(); return fn; }
function runSyncs() { for (const fn of [...syncs]) fn(); }
/** Runs `build` and returns the sync functions it registered, so a list can drop them when it redraws. */
function scoped(build) {
  const before = syncs.length;
  build();
  return syncs.slice(before);
}
function dropSyncs(list) {
  for (const fn of list) {
    const i = syncs.indexOf(fn);
    if (i >= 0) syncs.splice(i, 1);
  }
}

function change(path, value) {
  setLocal(path, value);
  runSyncs();
  throttleBy(path, 40, () => send({ t: 'set', path, value: get(path) }));
}

function requestIcons() {
  clearTimeout(iconTimer);
  iconTimer = setTimeout(() => {
    const ids = [...pendingIcons];
    pendingIcons.clear();
    if (ids.length) send({ t: 'icons', ids });
  }, 30);
}
function onIcons(map) {
  Object.assign(iconCache, map);
  for (const img of document.querySelectorAll('img[data-id]')) {
    const url = iconCache[img.dataset.id];
    if (url && img.getAttribute('src') !== url) img.setAttribute('src', url);
  }
}
function iconImg(id) {
  const img = h('img', { class: 'app-icon', alt: '', 'data-id': id });
  if (iconCache[id]) img.setAttribute('src', iconCache[id]);
  else { pendingIcons.add(id); requestIcons(); }
  return img;
}

// ---------------------------------------------------------------- shortcuts

const MOD_ORDER = ['ctrl', 'opt', 'shift', 'cmd'];
const MOD_ALIASES = { command: 'cmd', control: 'ctrl', option: 'opt', alt: 'opt' };
const KEY_ALIASES = { enter: 'return', escape: 'esc' };
const SYMBOLS = { ctrl: '⌃', opt: '⌥', shift: '⇧', cmd: '⌘', left: '←', right: '→', up: '↑', down: '↓', return: '↩', space: 'Space', tab: '⇥', esc: '⎋' };
const MODIFIER_CODES = new Set(['ShiftLeft', 'ShiftRight', 'ControlLeft', 'ControlRight', 'AltLeft', 'AltRight', 'MetaLeft', 'MetaRight', 'CapsLock', 'Fn']);

/** The key name the app understands for a KeyboardEvent.code, or null when it does not support it. */
function keyName(code) {
  let m;
  if ((m = /^Key([A-Z])$/.exec(code))) return m[1].toLowerCase();
  if ((m = /^Digit(\d)$/.exec(code))) return m[1];
  if ((m = /^F(\d{1,2})$/.exec(code)) && +m[1] >= 1 && +m[1] <= 20) return 'f' + m[1];
  return { ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down', Enter: 'return', NumpadEnter: 'return', Space: 'space', Tab: 'tab' }[code] || null;
}

/** "cmd+shift+c" as the parts to draw: modifiers in the order macOS writes them, then the key. */
function shortcutParts(shortcut) {
  const parts = String(shortcut || '').split('+').map((p) => p.trim().toLowerCase()).filter(Boolean)
    .map((p) => MOD_ALIASES[p] || KEY_ALIASES[p] || p);
  return [...MOD_ORDER.filter((m) => parts.includes(m)), ...parts.filter((p) => !MOD_ORDER.includes(p))];
}
function keyLabel(part) { return SYMBOLS[part] || part.toUpperCase(); }
function keycapsEl(shortcut) {
  return h('span', { class: 'keys' }, shortcutParts(shortcut).map((p) => h('kbd', { text: keyLabel(p) })));
}

function shortcutOf(target) {
  if (target === 'clipboard') return S.clipboard.hotkey;
  const key = target.split(':')[1];
  return (S.tiling.layouts.find((l) => l.key === key) || {}).shortcut || '';
}

let rec = null; // the shortcut button that is capturing a key combination

function stopRecording() {
  if (!rec) return;
  const { el, render } = rec;
  rec = null;
  el.classList.remove('recording');
  window.removeEventListener('keydown', onRecordKey, true);
  window.removeEventListener('mousedown', onRecordAway, true);
  send({ t: 'recording', on: false });
  render();
}
function startRecording(el, target, allowClear, render) {
  if (rec) stopRecording();
  rec = { el, target, allowClear, render };
  el.classList.add('recording');
  send({ t: 'recording', on: true });
  render();
  window.addEventListener('keydown', onRecordKey, true);
  window.addEventListener('mousedown', onRecordAway, true);
}
function onRecordAway(e) { if (rec && !rec.el.contains(e.target)) stopRecording(); }
function onRecordKey(e) {
  e.preventDefault();
  e.stopPropagation();
  if (MODIFIER_CODES.has(e.code)) return;
  const noMods = !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey;
  if (e.code === 'Escape') return stopRecording();
  if ((e.code === 'Backspace' || e.code === 'Delete') && noMods && rec.allowClear) {
    send({ t: 'hotkey', target: rec.target, value: '' });
    return stopRecording();
  }
  const key = keyName(e.code);
  if (!key) return toast(t('fault.unsupported_key'));
  const mods = [];
  if (e.ctrlKey) mods.push('ctrl');
  if (e.altKey) mods.push('opt');
  if (e.shiftKey) mods.push('shift');
  if (e.metaKey) mods.push('cmd');
  if (!mods.length && !/^f\d+$/.test(key)) return toast(t('fault.needs_modifier'));
  send({ t: 'hotkey', target: rec.target, value: [...mods, key].join('+') });
  stopRecording();
}

// ---------------------------------------------------------------- feedback

let toastTimer = 0;
function toast(text) {
  const el = $('#toast');
  el.replaceChildren(h('div', { class: 'glass', role: 'status', text }));
  el.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), 3600);
}

function onFault(m) {
  let text;
  if (m.code === 'conflict') text = t('fault.conflict', { name: targetName(m.detail) });
  else if (m.code === 'login_failed') text = t('fault.login_failed', { detail: m.detail });
  else text = t('fault.' + m.code);
  toast(text);
}
function targetName(target) {
  if (target === 'clipboard') return t('target.clipboard');
  return t('layout.' + String(target).replace('tiling:', ''));
}

function closeModal() {
  $('#modal').classList.remove('show');
  window.removeEventListener('keydown', modalKey, true);
}
function modalKey(e) { if (e.key === 'Escape') { e.stopPropagation(); closeModal(); } }
function confirmDialog({ title, body, confirm }, onYes) {
  const cancel = h('button', { class: 'btn', text: t('cancel'), onclick: closeModal });
  const yes = h('button', { class: 'btn primary', text: confirm, onclick: () => { closeModal(); onYes(); } });
  $('#modal').replaceChildren(h('div', { class: 'dialog glass', role: 'dialog', 'aria-modal': 'true', 'aria-label': title },
    h('h3', { text: title }), h('p', { text: body }), h('div', { class: 'actions' }, cancel, yes)));
  $('#modal').classList.add('show');
  window.addEventListener('keydown', modalKey, true);
  cancel.focus();
}

// ---------------------------------------------------------------- components

function switchEl(path, label) {
  const el = h('button', { class: 'switch', role: 'switch', 'aria-checked': 'false', 'aria-label': label || '', onclick: () => change(path, !get(path)) });
  reg(() => el.setAttribute('aria-checked', String(!!get(path))));
  return el;
}

function row(title, desc, control, opts = {}) {
  const el = h('div', { class: 'row' + (opts.block ? ' block' : '') },
    h('div', { class: 'row-text' }, h('div', { class: 'row-title' }, title), desc && h('div', { class: 'row-desc', text: desc })),
    control && h('div', { class: 'row-control' }, control));
  if (opts.when) reg(() => el.classList.toggle('disabled', !opts.when()));
  return el;
}
function toggleRow(path, title, desc, opts) { return row(title, desc, switchEl(path, title), opts); }
function card(...rows) { return h('section', { class: 'card glass' }, rows); }
function sectionTitle(text) { return h('div', { class: 'section-title', text }); }

function segmented({ options, value, onPick, small, label }) {
  const thumb = h('span', { class: 'seg-thumb' });
  const buttons = options.map((o) => h('button', { role: 'radio', 'aria-checked': 'false', text: o.label, onclick: () => onPick(o.value) }));
  const el = h('div', { class: 'seg' + (small ? ' small' : ''), role: 'radiogroup', 'aria-label': label || '', style: `--n:${options.length}` }, thumb, buttons);
  reg(() => {
    const current = value();
    const i = Math.max(0, options.findIndex((o) => o.value === current));
    el.style.setProperty('--i', i);
    buttons.forEach((b, k) => b.setAttribute('aria-checked', String(k === i)));
  });
  return el;
}

function sliderRow({ path, title, desc, min, max, step, format, onInput }) {
  const input = h('input', { type: 'range', min, max, step, 'aria-label': title });
  const out = h('output', { 'aria-live': 'off' });
  let dragging = false;
  input.addEventListener('pointerdown', () => { dragging = true; });
  window.addEventListener('pointerup', () => { dragging = false; });
  const paint = () => {
    const v = +input.value;
    input.style.setProperty('--p', ((v - min) / (max - min)) * 100 + '%');
    out.textContent = format(v);
  };
  input.addEventListener('input', () => { paint(); change(path, +input.value); if (onInput) onInput(); });
  reg(() => { if (!dragging) input.value = get(path); paint(); });
  return row(title, desc, h('div', { class: 'slider' }, input, out));
}

function button(label, onclick, opts = {}) {
  return h('button', { class: 'btn' + (opts.primary ? ' primary' : '') + (opts.danger ? ' danger' : '') + (opts.small ? ' small' : ''), onclick, 'aria-label': opts.aria },
    opts.icon && icon(opts.icon), label);
}

function hotkeyButton(target, opts = {}) {
  const el = h('button', { class: 'hotkey' });
  const render = () => {
    if (rec && rec.el === el) { el.replaceChildren(h('span', { class: 'hint', text: t('rec.press') })); return; }
    const shortcut = shortcutOf(target);
    el.replaceChildren(shortcut ? keycapsEl(shortcut) : h('span', { class: 'none', text: t('tiling.none') }));
    el.setAttribute('aria-label', (opts.label || '') + ' ' + (shortcut ? shortcutParts(shortcut).map(keyLabel).join(' ') : t('tiling.none')));
  };
  el.addEventListener('click', () => startRecording(el, target, !!opts.allowClear, render));
  reg(render);
  return el;
}

// App pickers -------------------------------------------------------

let menuEl = null;
let pendingMenu = null;
function closeMenu() {
  if (menuEl) { menuEl.remove(); menuEl = null; }
  window.removeEventListener('mousedown', menuAway, true);
  window.removeEventListener('keydown', menuKey, true);
}
function menuAway(e) { if (menuEl && !menuEl.contains(e.target)) closeMenu(); }
function menuKey(e) { if (e.key === 'Escape') { e.stopPropagation(); closeMenu(); } }

function addAppButton(purpose, label) {
  const btn = h('button', { class: 'btn small', onclick: () => { pendingMenu = { anchor: btn, purpose }; send({ t: 'running', purpose }); } }, icon('plus'), label);
  return btn;
}
function onRunning(m) {
  if (!pendingMenu || pendingMenu.purpose !== m.purpose) return;
  const { anchor, purpose } = pendingMenu;
  pendingMenu = null;
  const listed = new Set((purpose === 'scroll' ? S.mouse.apps : S.clipboard.ignored).map((a) => a.id));
  const free = m.apps.filter((a) => !listed.has(a.id));
  closeMenu();
  const addMessage = (id) => ({ t: purpose === 'scroll' ? 'app_add' : 'ignored_add', id });
  menuEl = h('div', { class: 'menu glass', role: 'menu' },
    h('div', { class: 'section-title', style: 'margin:8px 10px 6px', text: t('menu.running') }),
    free.length
      ? free.map((a) => h('button', { role: 'menuitem', onclick: () => { closeMenu(); send(addMessage(a.id)); } }, iconImg(a.id), a.name || a.id))
      : h('div', { class: 'empty', text: t('menu.none') }),
    h('hr'),
    h('button', { role: 'menuitem', onclick: () => { closeMenu(); send({ t: 'pick_app', purpose }); } }, icon('folder', 'menu-icon'), t('menu.disk')));
  document.body.append(menuEl);
  const rect = anchor.getBoundingClientRect();
  const width = menuEl.offsetWidth;
  const height = menuEl.offsetHeight;
  menuEl.style.left = clamp(rect.right - width, 8, innerWidth - width - 8) + 'px';
  const below = rect.bottom + 6;
  menuEl.style.top = (below + height > innerHeight - 8 ? Math.max(8, rect.top - height - 6) : below) + 'px';
  window.addEventListener('mousedown', menuAway, true);
  window.addEventListener('keydown', menuKey, true);
}

function appRow(app, controls) {
  return h('div', { class: 'app-row' }, iconImg(app.id),
    h('div', { class: 'app-text' }, h('div', { class: 'app-name', text: app.name }), h('div', { class: 'app-id', text: app.id })),
    controls);
}

/** A list that redraws only when its data changes, dropping the controls of the rows it replaces. */
function dynamicList(read, build, emptyText) {
  const box = h('div', { class: 'apps' });
  let last = null;
  let mine = [];
  reg(() => {
    const data = read();
    const key = JSON.stringify(data) + lang;
    if (key === last) return;
    last = key;
    dropSyncs(mine);
    mine = scoped(() => {
      box.replaceChildren(...(data.length ? data.map(build) : [h('div', { class: 'empty', text: emptyText })]));
    });
  });
  return box;
}

function ruleRow(rule) {
  const current = (field) => () => (S.mouse.apps.find((a) => a.id === rule.id) || {})[field] ?? null;
  const tri = (field, label) => segmented({
    small: true, label,
    options: [{ value: null, label: t('rule.default') }, { value: true, label: t('rule.on') }, { value: false, label: t('rule.off') }],
    value: current(field),
    onPick: (value) => {
      const entry = S.mouse.apps.find((a) => a.id === rule.id);
      if (entry) entry[field] = value;
      runSyncs();
      send({ t: 'app_rule', id: rule.id, field, value });
    },
  });
  return appRow(rule, h('div', { class: 'app-opts' },
    h('label', {}, t('apps.smooth'), tri('smooth', t('apps.smooth'))),
    h('label', {}, t('apps.reverse'), tri('reverse', t('apps.reverse'))),
    h('button', { class: 'btn icon', 'aria-label': t('apps.remove'), title: t('apps.remove'), onclick: () => send({ t: 'app_remove', id: rule.id }) }, icon('close'))));
}
function ignoredRow(app) {
  return appRow(app, h('button', { class: 'btn icon', 'aria-label': t('apps.remove'), title: t('apps.remove'), onclick: () => send({ t: 'ignored_remove', id: app.id }) }, icon('close')));
}

// ---------------------------------------------------------------- pages

function pill(textFn, goodFn) {
  const dot = h('span', { class: 'dot' });
  const label = h('span');
  const el = h('span', { class: 'chip' }, dot, label);
  reg(() => { label.textContent = textFn(); dot.classList.toggle('good', goodFn()); });
  return el;
}

function pageGeneral() {
  const hero = h('section', { class: 'hero card glass' },
    h('div', { class: 'hero-mark', html: GHOST }),
    h('div', {},
      h('h2', { text: 'Figghost' }),
      h('p', { text: t('hero.tagline') }),
      h('div', { class: 'chips' },
        h('span', { class: 'chip', text: t('chip.version', { v: S.app.version }) }),
        pill(() => (S.env.accessibility ? t('chip.access.ok') : t('chip.access.no')), () => S.env.accessibility))));

  // Launch at login depends on how the app was installed, so its row has more states than a plain switch.
  const loginOn = () => S.env.login === 'enabled' || S.env.login === 'needs_approval';
  const loginSwitch = h('button', {
    class: 'switch', role: 'switch', 'aria-label': t('login.title'),
    onclick: () => send({ t: 'set', path: 'launch_at_login', value: !loginOn() }),
  });
  const loginOpen = button(t('login.open'), () => send({ t: 'action', name: 'open_login_settings' }), { small: true });
  const loginRow = row(t('login.title'), t('login.desc'), [loginOpen, loginSwitch]);
  const loginDesc = $('.row-desc', loginRow);
  reg(() => {
    loginSwitch.setAttribute('aria-checked', String(loginOn()));
    loginOpen.style.display = S.env.login === 'needs_approval' ? '' : 'none';
    loginDesc.textContent = S.env.login === 'needs_approval' ? t('login.approval')
      : S.env.login === 'unavailable' ? t('login.unavailable') : t('login.desc');
    loginSwitch.disabled = S.env.login === 'unavailable';
    loginRow.classList.toggle('disabled', S.env.login === 'unavailable');
  });

  const restart = h('div', { class: 'banner' }, h('div', { class: 'grow', text: t('lang.restart') }),
    button(t('lang.restart.btn'), () => send({ t: 'action', name: 'restart' }), { primary: true, small: true }));
  reg(() => { restart.style.display = resolveLang(S.general.language) !== S.env.language ? '' : 'none'; });

  const access = row(t('access.title'), t('access.desc'), [
    pill(() => (S.env.accessibility ? t('access.granted') : t('access.missing')), () => S.env.accessibility),
    button(t('access.open'), () => send({ t: 'action', name: 'open_accessibility' }), { small: true }),
  ]);

  return [
    hero,
    sectionTitle(t('sec.startup')),
    card(loginRow, toggleRow('general.show_on_launch', t('showlaunch.title'), t('showlaunch.desc'))),
    sectionTitle(t('sec.language')),
    card(row(t('lang.title'), t('lang.desc'), segmented({
      label: t('lang.title'),
      options: [{ value: 'auto', label: t('lang.auto') }, { value: 'en', label: t('lang.en') }, { value: 'vi', label: t('lang.vi') }],
      value: () => S.general.language,
      onPick: (v) => { change('general.language', v); applyLanguage(); },
    }))),
    restart,
    sectionTitle(t('sec.permissions')),
    card(access),
    sectionTitle(t('sec.data')),
    card(row(t('data.title'), t('data.desc'), button(t('data.open'), () => send({ t: 'action', name: 'open_config' }), { icon: 'folder', small: true }))),
  ];
}

// -- scrolling ------------------------------------------------------

let curve = { duration: 0, points: [] }; // from the app: [[ms, share covered, relative speed], ...]
const graphs = new Set();
function requestCurve() { throttleBy('curve', 30, () => send({ t: 'curve', duration_ms: S.mouse.duration_ms })); }
function onCurve(m) { curve = { duration: m.duration_ms, points: m.points }; for (const draw of graphs) draw(); }

function cssVar(name) { return getComputedStyle(document.documentElement).getPropertyValue(name).trim(); }

function drawCurve(canvas) {
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth, hgt = canvas.clientHeight;
  if (!w || !hgt) return;
  canvas.width = Math.round(w * dpr);
  canvas.height = Math.round(hgt * dpr);
  const ctx = canvas.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, hgt);
  const pts = curve.points;
  if (!pts.length) return;

  const m = { l: 46, r: 12, t: 12, b: 26 };
  const pw = w - m.l - m.r, ph = hgt - m.t - m.b;
  const total = S.mouse.step_px * S.mouse.speed;
  const tMax = pts[pts.length - 1][0];
  const vMax = pts[0][2];
  const x = (ms) => m.l + (ms / tMax) * pw;
  const yDist = (share) => m.t + ph - share * ph;
  const accent = cssVar('--accent'), accent2 = cssVar('--accent-2'), text3 = cssVar('--text-3'), stroke = cssVar('--stroke');

  ctx.font = '11px ' + cssVar('--mono');
  ctx.textBaseline = 'middle';
  ctx.lineWidth = 1;
  for (let i = 0; i <= 4; i++) {
    const share = i / 4, y = yDist(share);
    ctx.strokeStyle = stroke;
    ctx.beginPath(); ctx.moveTo(m.l, y); ctx.lineTo(w - m.r, y); ctx.stroke();
    ctx.fillStyle = text3; ctx.textAlign = 'right';
    ctx.fillText(Math.round(share * total) + '', m.l - 8, y);
  }
  ctx.textAlign = 'center'; ctx.textBaseline = 'top';
  const ticks = 5;
  for (let i = 0; i <= ticks; i++) {
    const ms = (tMax / ticks) * i;
    ctx.fillStyle = text3;
    ctx.fillText(Math.round(ms) + ' ms', clamp(x(ms), m.l + 14, w - m.r - 14), hgt - m.b + 8);
  }

  // Share of the distance covered so far.
  const fill = ctx.createLinearGradient(0, m.t, 0, m.t + ph);
  fill.addColorStop(0, accent + '55');
  fill.addColorStop(1, accent + '04');
  ctx.beginPath();
  pts.forEach(([ms, share], i) => (i ? ctx.lineTo(x(ms), yDist(share)) : ctx.moveTo(x(ms), yDist(share))));
  ctx.lineTo(x(tMax), yDist(0)); ctx.lineTo(x(0), yDist(0)); ctx.closePath();
  ctx.fillStyle = fill; ctx.fill();
  ctx.beginPath();
  pts.forEach(([ms, share], i) => (i ? ctx.lineTo(x(ms), yDist(share)) : ctx.moveTo(x(ms), yDist(share))));
  ctx.strokeStyle = accent; ctx.lineWidth = 2.5; ctx.lineJoin = 'round'; ctx.stroke();

  // Speed, as a dashed line scaled to the same height.
  ctx.beginPath();
  pts.forEach(([ms, , speed], i) => {
    const y = m.t + ph - (speed / vMax) * ph * 0.92;
    i ? ctx.lineTo(x(ms), y) : ctx.moveTo(x(ms), y);
  });
  ctx.setLineDash([5, 4]); ctx.strokeStyle = accent2; ctx.lineWidth = 2; ctx.stroke(); ctx.setLineDash([]);

  // Where the glide is 98% done.
  const done = pts.find(([, share]) => share >= 0.98);
  if (done) {
    ctx.strokeStyle = text3; ctx.setLineDash([2, 4]);
    ctx.beginPath(); ctx.moveTo(x(done[0]), m.t); ctx.lineTo(x(done[0]), m.t + ph); ctx.stroke(); ctx.setLineDash([]);
    ctx.fillStyle = text3; ctx.textAlign = 'center'; ctx.textBaseline = 'bottom';
    ctx.fillText('98%', x(done[0]), m.t + 12);
  }
}

function curveCard() {
  const canvas = h('canvas', { role: 'img', 'aria-label': t('graph.distance') + ' / ' + t('graph.velocity') });
  const summary = h('span');
  const legend = h('div', { class: 'graph-legend' },
    h('span', {}, h('b', { style: 'background:var(--accent)' }), t('graph.distance')),
    h('span', {}, h('b', { style: 'background:var(--accent-2)' }), t('graph.velocity')),
    h('span', { style: 'margin-left:auto' }, summary));
  const draw = () => drawCurve(canvas);
  graphs.add(draw);
  new ResizeObserver(draw).observe(canvas);
  reg(() => {
    if (Math.abs(curve.duration - S.mouse.duration_ms) > 0.5) requestCurve();
    const done = curve.points.find(([, share]) => share >= 0.98);
    summary.textContent = t('graph.per', { px: Math.round(S.mouse.step_px * S.mouse.speed), ms: done ? Math.round(done[0]) : Math.round(S.mouse.duration_ms * 1.0) });
    draw();
  });
  return h('section', { class: 'card glass graph' }, canvas, legend);
}

const PRESETS = {
  precise: { step_px: 28, speed: 0.9, duration_ms: 220 },
  balanced: { step_px: 36, speed: 1, duration_ms: 360 },
  fluid: { step_px: 48, speed: 1.15, duration_ms: 560 },
};
function applyPreset(values) { for (const [key, value] of Object.entries(values)) change('mouse.' + key, value); }
function presetsRow() {
  const near = (a, b) => Math.abs(a - b) < 0.001;
  const matches = (p) => near(S.mouse.step_px, p.step_px) && near(S.mouse.speed, p.speed) && near(S.mouse.duration_ms, p.duration_ms);
  const buttons = Object.entries(PRESETS).map(([name, values]) => {
    const el = h('button', { class: 'btn small', text: t('preset.' + name), onclick: () => applyPreset(values) });
    reg(() => el.classList.toggle('primary', matches(values)));
    return el;
  });
  return h('div', { class: 'row' }, h('div', { class: 'row-text' }, h('div', { class: 'presets' }, buttons)));
}

function scrollPad() {
  const lines = Array.from({ length: 150 }, (_, i) => h('div', { class: 'pad-line', text: t('pad.line', { n: i + 1 }) }));
  const pad = h('div', { class: 'pad', tabindex: '0', role: 'region', 'aria-label': t('pad.hint') }, lines);
  const meter = h('canvas', { role: 'img', 'aria-label': t('pad.speed') });
  const values = { speed: h('b', { text: '0' }), peak: h('b', { text: '0' }), frames: h('b', { text: '0' }), jump: h('b', { text: '0' }) };
  const stat = (key, label) => h('div', { class: 'stat' }, h('span', { text: label }), values[key]);

  let last = { top: 0, at: 0 }, running = false, idleSince = 0, deltas = [];
  const stats = { peak: 0, frames: 0, jump: 0 };
  const paint = () => {
    const dpr = window.devicePixelRatio || 1, w = meter.clientWidth, hgt = meter.clientHeight;
    if (!w) return;
    meter.width = Math.round(w * dpr); meter.height = Math.round(hgt * dpr);
    const ctx = meter.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const peak = Math.max(20, ...deltas);
    const bar = w / 60;
    deltas.forEach((d, i) => {
      const bh = (d / peak) * (hgt - 8);
      ctx.fillStyle = cssVar('--accent');
      ctx.globalAlpha = 0.35 + 0.65 * (i / deltas.length);
      ctx.fillRect(i * bar + 1, hgt - 4 - bh, Math.max(1, bar - 2), bh);
    });
    ctx.globalAlpha = 1;
  };
  const frame = (now) => {
    const dt = Math.max(1, now - last.at) / 1000;
    const delta = Math.abs(pad.scrollTop - last.top);
    last = { top: pad.scrollTop, at: now };
    if (delta > 0) {
      idleSince = now;
      deltas.push(delta); if (deltas.length > 60) deltas.shift();
      stats.frames++; stats.jump = Math.max(stats.jump, delta);
      const speed = delta / dt; stats.peak = Math.max(stats.peak, speed);
      values.speed.textContent = Math.round(speed) + ' px/s';
    }
    values.peak.textContent = Math.round(stats.peak) + ' px/s';
    values.frames.textContent = String(stats.frames);
    values.jump.textContent = Math.round(stats.jump) + ' px';
    paint();
    if (now - idleSince < 500) requestAnimationFrame(frame); else running = false;
  };
  pad.addEventListener('scroll', () => {
    if (running) return;
    // A new gesture starts after a pause: begin the statistics again.
    deltas = []; stats.peak = 0; stats.frames = 0; stats.jump = 0;
    running = true; idleSince = performance.now(); last = { top: pad.scrollTop, at: performance.now() };
    requestAnimationFrame(frame);
  });
  new ResizeObserver(paint).observe(meter);
  return h('section', { class: 'card glass' },
    h('div', { class: 'pad-wrap' }, pad,
      h('div', { class: 'meter' }, meter, stat('speed', t('pad.speed')), stat('peak', t('pad.peak')), stat('frames', t('pad.frames')), stat('jump', t('pad.jump')),
        h('div', { class: 'row-desc', text: t('pad.hint') }))));
}

function modTitle(symbol, text) {
  return h('span', {}, h('kbd', { text: symbol, style: 'margin-right:9px' }), text);
}

function boostSelect() {
  const values = [2, 3, 5, 8, 12];
  const el = h('select', { class: 'pick', 'aria-label': t('boost.title') }, values.map((v) => h('option', { value: String(v), text: '× ' + v })));
  el.addEventListener('change', () => change('mouse.boost', +el.value));
  reg(() => {
    const current = String(S.mouse.boost);
    if (![...el.options].some((o) => o.value === current)) el.append(h('option', { value: current, text: '× ' + S.mouse.boost }));
    el.value = current;
    el.disabled = !S.mouse.option_boost;
  });
  return el;
}

function pageScroll() {
  return [
    sectionTitle(t('sec.wheel')),
    card(
      toggleRow('mouse.smooth', t('smooth.title'), t('smooth.desc')),
      toggleRow('mouse.reverse', t('reverse.title'), t('reverse.desc'))),
    sectionTitle(t('sec.feel')),
    curveCard(),
    card(
      sliderRow({ path: 'mouse.step_px', title: t('step.title'), desc: t('step.desc'), min: 8, max: 120, step: 1, format: (v) => Math.round(v) + ' px' }),
      sliderRow({ path: 'mouse.speed', title: t('speed.title'), desc: t('speed.desc'), min: 0.3, max: 3, step: 0.05, format: (v) => '× ' + v.toFixed(2) }),
      sliderRow({ path: 'mouse.duration_ms', title: t('duration.title'), desc: t('duration.desc'), min: 80, max: 1000, step: 10, format: (v) => Math.round(v) + ' ms' }),
      presetsRow()),
    sectionTitle(t('sec.try')),
    scrollPad(),
    sectionTitle(t('sec.mods')),
    card(
      toggleRow('mouse.shift_horizontal', modTitle('⇧', t('shift.title')), t('shift.desc')),
      row(modTitle('⌥', t('option.title')), t('option.desc'), [boostSelect(), switchEl('mouse.option_boost', t('option.title'))]),
      toggleRow('mouse.command_zoom', modTitle('⌘', t('cmd.title')), t('cmd.desc'))),
    sectionTitle(t('sec.apps')),
    card(
      row(t('sec.apps'), t('apps.desc'), addAppButton('scroll', t('apps.add'))),
      dynamicList(() => S.mouse.apps, ruleRow, t('apps.empty'))),
  ];
}

// -- clipboard ------------------------------------------------------

function limitSelect() {
  const values = [100, 250, 500, 1000, 5000, 20000];
  const el = h('select', { class: 'pick', 'aria-label': t('limit.title') }, values.map((v) => h('option', { value: String(v), text: v.toLocaleString() })));
  el.addEventListener('change', () => change('clipboard.history_limit', +el.value));
  reg(() => {
    const current = String(S.clipboard.history_limit);
    if (![...el.options].some((o) => o.value === current)) el.append(h('option', { value: current, text: (+current).toLocaleString() }));
    el.value = current;
  });
  return el;
}

function cheat(keys, text) {
  return h('div', { class: 'cheat' }, h('span', { text }), h('span', { class: 'keys' }, keys.map((k) => h('kbd', { text: k }))));
}

function pageClipboard() {
  const count = h('span', { class: 'row-title' });
  reg(() => { count.textContent = S.clipboard.count.toLocaleString(); });
  return [
    sectionTitle(t('sec.shortcut')),
    card(row(t('clip.hotkey.title'), t('clip.hotkey.desc'), hotkeyButton('clipboard', { label: t('clip.hotkey.title') }))),
    sectionTitle(t('sec.pasting')),
    card(
      toggleRow('clipboard.auto_paste', t('paste.auto.title'), t('paste.auto.desc')),
      toggleRow('clipboard.paste_plain', t('paste.plain.title'), t('paste.plain.desc'))),
    sectionTitle(t('sec.history')),
    card(
      row(t('limit.title'), t('limit.desc'), limitSelect()),
      row(t('count.title'), null, count),
      row(t('clear.title'), t('clear.desc'), button(t('clear.btn'), () => confirmDialog({ title: t('clear.confirm.title'), body: t('clear.confirm.body'), confirm: t('clear.btn') }, () => send({ t: 'action', name: 'clear_history' })), { icon: 'trash', small: true, danger: true }))),
    sectionTitle(t('sec.privacy')),
    card(
      row(t('ignored.title'), t('ignored.desc'), addAppButton('ignore', t('ignored.add'))),
      dynamicList(() => S.clipboard.ignored, ignoredRow, t('ignored.empty'))),
    sectionTitle(t('sec.keys')),
    h('section', { class: 'card glass' }, h('div', { class: 'cheats' },
      cheat(['↩'], t('key.paste')),
      cheat(['⌥', '⇧', '↩'], t('key.pastealt')),
      cheat(['⌥', 'P'], t('key.pin')),
      cheat(['⌥', '⌫'], t('key.delete')),
      cheat(['⌘', 'B…'], t('key.pinkey')),
      cheat(['⎋'], t('key.close')),
      h('div', { class: 'cheat' }, h('span', { text: t('key.filters') }), h('span', { class: 'row-title', style: 'font-family:var(--mono);font-size:12px', text: t('key.filters.val') })))),
  ];
}

// -- notch ----------------------------------------------------------

function notchStage() {
  return h('div', { class: 'notch-stage', 'aria-hidden': 'true' },
    h('div', { class: 'menubar' }),
    h('div', { class: 'notch' },
      h('div', { class: 'notch-body' },
        h('div', { class: 'notch-tabs' }, h('span', { text: t('tab.clipboard') }), h('span', { text: t('tab.shelf') }), h('span', { text: t('tab.media') })),
        h('div', { class: 'notch-line' }), h('div', { class: 'notch-line' }), h('div', { class: 'notch-line' }))),
    h('div', { class: 'notch-note', text: t('notch.preview') }));
}

function infoRow(title, desc) { return row(title, desc, null); }

function pageNotch() {
  const shelfCount = h('span', { class: 'row-title' });
  reg(() => { shelfCount.textContent = String(S.notch.shelf_count); });
  return [
    notchStage(),
    card(toggleRow('notch.enabled', t('notch.panel.title'), t('notch.panel.desc'))),
    sectionTitle(t('sec.tabs')),
    card(infoRow(t('tab.clipboard'), t('tab.clipboard.desc')), infoRow(t('tab.shelf'), t('tab.shelf.desc')), infoRow(t('tab.media'), t('tab.media.desc'))),
    sectionTitle(t('sec.shelf')),
    card(
      row(t('shelf.count'), null, shelfCount),
      row(t('shelf.clear'), t('shelf.clear.desc'), button(t('shelf.clear'), () => send({ t: 'action', name: 'clear_shelf' }), { icon: 'trash', small: true }))),
    sectionTitle(t('sec.volume')),
    card(toggleRow('notch.volume_hud', t('vol.title'), t('vol.desc'))),
  ];
}

// -- window snapping ------------------------------------------------

const LAYOUT_RECTS = {
  left: [0, 0, 0.5, 1], right: [0.5, 0, 0.5, 1], top: [0, 0, 1, 0.5], bottom: [0, 0.5, 1, 0.5],
  top_left: [0, 0, 0.5, 0.5], top_right: [0.5, 0, 0.5, 0.5], bottom_left: [0, 0.5, 0.5, 0.5], bottom_right: [0.5, 0.5, 0.5, 0.5],
  maximize: [0, 0, 1, 1], center: [0.2, 0.18, 0.6, 0.64], first_third: [0, 0, 1 / 3, 1], center_third: [1 / 3, 0, 1 / 3, 1], last_third: [2 / 3, 0, 1 / 3, 1],
};

function layoutTile(layout) {
  const [x, y, w, hh] = LAYOUT_RECTS[layout.key];
  const rect = h('i', { style: `left:${x * 100}%;top:${y * 100}%;width:${w * 100}%;height:${hh * 100}%` });
  const target = 'tiling:' + layout.key;
  const clear = h('button', { class: 'btn icon', 'aria-label': t('tiling.clear'), title: t('tiling.clear'), onclick: () => send({ t: 'hotkey', target, value: '' }) }, icon('close'));
  reg(() => { clear.style.visibility = shortcutOf(target) ? 'visible' : 'hidden'; });
  return h('div', { class: 'tile' },
    h('div', { class: 'tile-top' }, h('div', { class: 'mini' }, rect), h('div', { class: 'tile-name', text: t('layout.' + layout.key) })),
    h('div', { class: 'tile-actions' }, hotkeyButton(target, { allowClear: true, label: t('layout.' + layout.key) }), clear));
}

function pageTiling() {
  const grid = h('section', { class: 'card glass' }, h('div', { class: 'tiles' }, S.tiling.layouts.map(layoutTile)));
  reg(() => grid.classList.toggle('dim', !S.tiling.enabled));
  return [
    card(toggleRow('tiling.enabled', t('tiling.enable.title'), t('tiling.enable.desc'))),
    grid,
    h('div', {}, button(t('tiling.reset'), () => send({ t: 'action', name: 'reset_tiling' }), { icon: 'refresh', small: true })),
  ];
}

// -- about ----------------------------------------------------------

function pageAbout() {
  return [
    h('section', { class: 'hero card glass' },
      h('div', { class: 'hero-mark', html: GHOST }),
      h('div', {}, h('h2', { text: 'Figghost' }), h('p', { text: t('about.line') }))),
    card(
      row(t('about.version'), null, h('span', { class: 'row-title', text: S.app.version })),
      row(t('about.privacy'), t('about.privacy.val'), null),
      row(t('data.title'), S.app.config_dir, button(t('data.open'), () => send({ t: 'action', name: 'open_config' }), { icon: 'folder', small: true }))),
    card(row(t('about.quit'), t('about.quit.desc'), button(t('about.quit'), () => send({ t: 'action', name: 'quit' }), { icon: 'power', small: true, danger: true }))),
  ];
}

const PAGES = [
  { id: 'general', icon: 'general', build: pageGeneral },
  { id: 'scroll', icon: 'scroll', build: pageScroll },
  { id: 'clipboard', icon: 'clipboard', build: pageClipboard },
  { id: 'notch', icon: 'notch', build: pageNotch },
  { id: 'tiling', icon: 'tiling', build: pageTiling },
  { id: 'about', icon: 'about', build: pageAbout },
];

// ---------------------------------------------------------------- shell

let current = 'general';
let shellSyncs = [];
let pageSyncs = [];

function accessPill() {
  const dot = h('span', { class: 'dot' });
  const label = h('span');
  const el = h('button', { class: 'status-pill', onclick: () => (S.env.accessibility ? go('general') : send({ t: 'action', name: 'open_accessibility' })) }, dot, label);
  reg(() => {
    label.textContent = S.env.accessibility ? t('chip.access.ok') : t('chip.access.no');
    dot.classList.toggle('good', !!S.env.accessibility);
  });
  return el;
}

function buildShell() {
  document.documentElement.lang = lang;
  $('#root').replaceChildren(
    h('div', { id: 'aurora', 'aria-hidden': 'true' }, h('i'), h('i'), h('i')),
    h('div', { id: 'app' },
      h('aside', { id: 'sidebar', class: 'glass' },
        h('div', { class: 'dragzone', 'data-drag': '' }),
        h('div', { class: 'brand' },
          h('div', { class: 'brand-mark', html: GHOST }),
          h('div', {}, h('div', { class: 'brand-name', text: 'Figghost' }), h('div', { class: 'brand-ver', text: 'v' + S.app.version }))),
        h('nav', { id: 'nav', 'aria-label': 'Figghost' },
          PAGES.map((p) => h('button', { class: 'nav-item', 'data-page': p.id, onclick: () => go(p.id) }, icon(p.icon), t('nav.' + p.id)))),
        h('div', { class: 'side-foot' }, accessPill())),
      h('main', { id: 'main' },
        h('div', { class: 'dragzone', 'data-drag': '' }),
        h('header', { class: 'page-head' }, h('h1', { class: 'page-title', id: 'title' }), h('p', { class: 'page-sub', id: 'subtitle' })),
        h('div', { id: 'scroller' }))),
    h('div', { id: 'toast', 'aria-live': 'polite' }),
    h('div', { id: 'modal' }));
}

function go(id) {
  stopRecording();
  closeMenu();
  const page = PAGES.find((p) => p.id === id) || PAGES[0];
  current = page.id;
  dropSyncs(pageSyncs);
  graphs.clear();
  $('#title').textContent = t('page.' + page.id);
  $('#subtitle').textContent = t('page.' + page.id + '.sub');
  const scroller = $('#scroller');
  pageSyncs = scoped(() => scroller.replaceChildren(h('div', { class: 'page' }, page.build())));
  scroller.scrollTop = 0;
  for (const item of document.querySelectorAll('.nav-item')) {
    if (item.dataset.page === current) item.setAttribute('aria-current', 'page'); else item.removeAttribute('aria-current');
  }
  send({ t: 'page', id: current });
}

function rebuild() {
  dropSyncs(shellSyncs);
  shellSyncs = scoped(buildShell);
  go(current);
}

function applyLanguage() {
  const next = resolveLang(S.general.language);
  if (next === lang) return;
  lang = next;
  rebuild();
}

function applyDisplay() {
  if (!S) return;
  const list = document.documentElement.classList;
  list.toggle('reduce-transparency', !!S.env.reduce_transparency);
  list.toggle('reduce-motion', !!S.env.reduce_motion);
  list.toggle('increase-contrast', !!S.env.increase_contrast);
  // The drifting colour is the only thing that animates on its own; stop it when nobody can see it.
  list.toggle('still', !!S.env.reduce_motion || document.hidden);
}

function onState(next) {
  const first = S === null;
  const before = lang;
  S = next;
  lang = resolveLang(S.general.language);
  applyDisplay();
  if (first) {
    shellSyncs = scoped(buildShell);
    go('general');
  } else if (lang !== before) {
    rebuild();
  } else {
    runSyncs();
  }
}

window.__fig = {
  recv(message) {
    switch (message.t) {
      case 'state': onState(message.state); break;
      case 'fault': onFault(message); break;
      case 'running': onRunning(message); break;
      case 'icons': onIcons(message.icons); break;
      case 'curve': onCurve(message); break;
    }
  },
};

// Light that follows the pointer across a glass surface.
let sheenFrame = 0;
let sheenEvent = null;
document.addEventListener('pointermove', (e) => {
  sheenEvent = e;
  if (sheenFrame) return;
  sheenFrame = requestAnimationFrame(() => {
    sheenFrame = 0;
    const glass = sheenEvent.target.closest && sheenEvent.target.closest('.glass');
    if (!glass) return;
    const r = glass.getBoundingClientRect();
    glass.style.setProperty('--mx', sheenEvent.clientX - r.left + 'px');
    glass.style.setProperty('--my', sheenEvent.clientY - r.top + 'px');
  });
});

// The title area moves and zooms the window, like a native title bar.
document.addEventListener('mousedown', (e) => { if (e.button === 0 && e.target.closest('[data-drag]')) window.fig.drag(); });
document.addEventListener('dblclick', (e) => { if (e.target.closest('[data-drag]')) window.fig.zoom(); });
window.addEventListener('blur', () => document.documentElement.classList.add('inactive'));
window.addEventListener('focus', () => document.documentElement.classList.remove('inactive'));
document.addEventListener('visibilitychange', applyDisplay);

// ---------------------------------------------------------------- self-test (run by the headless check)

window.__selftest = function () {
  const failures = [];
  const expect = (ok, message) => { if (!ok) failures.push(message); };
  const sent = [];
  const realSend = window.fig.send;
  window.fig.send = (m) => sent.push(m);
  try {
    // Both languages cover the same keys.
    const en = Object.keys(I18N.en), vi = Object.keys(I18N.vi);
    expect(en.filter((k) => !I18N.vi[k]).length === 0, 'missing in vi: ' + en.filter((k) => !I18N.vi[k]).join(', '));
    expect(vi.filter((k) => !I18N.en[k]).length === 0, 'missing in en: ' + vi.filter((k) => !I18N.en[k]).join(', '));

    // Shortcut helpers.
    expect(keyName('KeyC') === 'c' && keyName('Digit7') === '7' && keyName('F13') === 'f13' && keyName('ArrowLeft') === 'left', 'keyName basics');
    expect(keyName('Backquote') === null && keyName('F21') === null, 'keyName rejects unsupported keys');
    expect(shortcutParts('cmd+shift+c').join() === 'shift,cmd,c', 'shortcutParts order: ' + shortcutParts('cmd+shift+c'));
    expect(shortcutParts('control+option+left').join() === 'ctrl,opt,left', 'shortcutParts aliases');
    expect(shortcutParts('').length === 0, 'shortcutParts empty');

    // Every page builds in both languages, with no missing text and no unlabeled buttons.
    for (const code of ['en', 'vi']) {
      setLang(code);
      for (const page of PAGES) {
        go(page.id);
        const root = $('#scroller');
        expect(root.children.length > 0 && root.textContent.trim().length > 20, `${code}/${page.id}: page is empty`);
        for (const b of root.querySelectorAll('button')) {
          expect(b.textContent.trim() || b.getAttribute('aria-label'), `${code}/${page.id}: a button has no label`);
        }
        for (const i of root.querySelectorAll('input')) expect(i.getAttribute('aria-label'), `${code}/${page.id}: an input has no label`);
      }
    }
    expect(missingKeys.size === 0, 'keys without English text: ' + [...missingKeys].join(', '));
    setLang('en');

    // Controls send what the app expects.
    go('scroll');
    sent.length = 0;
    const smooth = [...document.querySelectorAll('.switch')][0];
    const was = S.mouse.smooth;
    smooth.click();
    expect(S.mouse.smooth === !was, 'the switch flips the local state');
    expect(sent.some((m) => m.t === 'set' && m.path === 'mouse.smooth' && m.value === !was), 'the switch sends a set message: ' + JSON.stringify(sent));
    smooth.click();
    const range = $('input[type=range]');
    range.value = String(Math.min(+range.max, 60));
    range.dispatchEvent(new Event('input'));
    expect(S.mouse.step_px === +range.value, 'the slider updates the local state');
    expect(range.style.getPropertyValue('--p') !== '', 'the slider paints its fill');

    // Presets set all three values.
    const fluid = [...document.querySelectorAll('.presets .btn')].find((b) => b.textContent === t('preset.fluid'));
    fluid.click();
    expect(S.mouse.step_px === 48 && S.mouse.duration_ms === 560, 'the fluid preset applies');

    // App rules and lists draw from the state.
    S.mouse.apps = [{ id: 'com.example.game', name: 'Game', smooth: false, reverse: null }];
    runSyncs();
    expect(document.querySelectorAll('.app-row').length === 1, 'a rule row is drawn');
    sent.length = 0;
    const remove = $('.app-row .btn.icon');
    remove.click();
    expect(sent.some((m) => m.t === 'app_remove' && m.id === 'com.example.game'), 'removing a rule sends app_remove');

    // Shortcut buttons start recording and ignore bare letters.
    go('clipboard');
    sent.length = 0;
    const hot = $('.hotkey');
    hot.click();
    expect(sent.some((m) => m.t === 'recording' && m.on === true), 'recording starts');
    window.dispatchEvent(Object.assign(new KeyboardEvent('keydown', { code: 'KeyV', bubbles: true, cancelable: true })));
    expect(!sent.some((m) => m.t === 'hotkey'), 'a bare letter is not accepted as a shortcut');
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyV', metaKey: true, altKey: true, bubbles: true, cancelable: true }));
    expect(sent.some((m) => m.t === 'hotkey' && m.target === 'clipboard' && m.value === 'opt+cmd+v'), 'a combination is sent: ' + JSON.stringify(sent));
    expect(sent.some((m) => m.t === 'recording' && m.on === false), 'recording stops');
    go('tiling');
    expect(document.querySelectorAll('.tile').length === 13, 'thirteen layouts are shown');
  } catch (error) {
    failures.push('exception: ' + (error && error.stack ? error.stack : error));
  } finally {
    window.fig.send = realSend;
  }
  return failures;
};

function setLang(code) {
  S.general.language = code;
  lang = code;
  rebuild();
}

send({ t: 'ready' });
