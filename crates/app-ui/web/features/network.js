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
    'network.drag': 'Drag from the ring of a connection to an app, or from the edge of an app to a connection. Wire Wi-Fi to Ethernet to combine them: an app wired to the big circle uses both, to a small one only that one. Drag the middle of a box or a circle to move it. Hold Space and drag to move around, hold Control and scroll to zoom.',
    'network.arrange': 'Arrange', 'network.arrange.link': 'By connection', 'network.arrange.traffic': 'By traffic', 'network.arrange.name': 'By name',
    'network.zoom.fit': 'Fit', 'network.zoom.reset': 'Zoom to 100%',
    'network.q.good': 'Strong: answers in {ms} ms', 'network.q.fair': 'Fair: answers in {ms} ms, {loss}% lost',
    'network.q.poor': 'Weak: answers in {ms} ms, {loss}% lost', 'network.q.down': 'No answer from the internet through it',
    'network.merge.split': 'Separate', 'network.merge.resize': 'Drag to make the circle larger or smaller', 'network.link.away': 'Not connected', 'network.app.idle': 'Not using the network now', 'network.hub.merge.all': 'Combined (every connection)',
    'network.card.target': 'Leaves by', 'network.card.priority': 'Priority', 'network.card.down': 'Most down (KB/s)', 'network.card.up': 'Most up (KB/s)', 'network.card.limit.none': 'No limit',
    'network.card.save': 'Save', 'network.card.unwire': 'Remove the wire', 'network.card.held': 'Held to {speed} for the apps above it',
    'network.priority.high': 'Main', 'network.priority.normal': 'Normal', 'network.priority.low': 'Below',
    'network.close': 'Close',
    'sec.network.metered': 'Save data',
    'network.metered.title': 'Save data on metered connections', 'network.metered.desc': 'On a connection you pay for by the byte, such as a phone\'s, only the apps you allow use it, and a cap can stop the rest. It applies to apps that follow the proxy settings, so Apply the wires must be on; other apps are only counted.',
    'network.metered.auto.title': 'Take a phone\'s connection for metered', 'network.metered.auto.desc': 'A connection named iPhone or Android, or with the address a phone gives when it shares its data, counts as metered. It is a guess: mark a connection by hand under Connections to correct it.',
    'network.metered.allow.title': 'Apps allowed on a metered connection', 'network.metered.allow.desc': 'Figghost itself is always allowed.', 'network.metered.allow.none': 'No app is allowed: every app that follows the proxy is held back.',
    'network.metered.allow.add': 'Add an app', 'network.metered.allow.remove': 'Remove {app}',
    'network.metered.cap.title': 'Data cap', 'network.metered.cap.desc': 'What the metered connections may move in a period, counted by Figghost. At the cap every app that follows the proxy is refused until the period ends.', 'network.metered.cap.none': 'No cap',
    'network.metered.period.title': 'Counted over', 'network.period.day': 'A day', 'network.period.month': 'A month',
    'network.metered.now': 'On a metered connection now ({link}): only the allowed apps go on.', 'network.metered.not': 'Not on a metered connection now.',
    'network.metered.cap.used': 'Used on metered connections this period: {used} of {cap}.', 'network.metered.cap.over': 'The cap is reached: every app that follows the proxy is refused.',
    'network.metered.mark': 'Metered', 'network.metered.mark.auto': 'Automatic', 'network.metered.mark.metered': 'Metered', 'network.metered.mark.not': 'Not metered',
    'network.hub.block': 'Nowhere (block)', 'network.blocked': 'Blocked', 'network.refused': '{n} refused',
    'network.card.quota': 'Data quota (MB)', 'network.card.quota.period': 'Counted over',
    'network.card.quota.used': 'Used {used} of {quota} this period',
    'network.card.hosts': 'Sites and addresses it talks to', 'network.card.hosts.none': 'Nothing seen yet. Apps that follow the proxy show the site name, the others show the address.',
    'network.card.block.note': 'Blocked: every connection this app opens through the proxy is refused.',
    'sec.network.anomaly': 'Sudden uploads',
    'network.anomaly.title': 'Tell me when an app uploads a lot', 'network.anomaly.desc': 'The notch says so when an app sends in five minutes more than 50 MB and more than ten times what it usually sends in an hour. Needs a day of history for the app, which Figghost counts for it.',
    'network.anomaly.ignore.title': 'Apps never reported', 'network.anomaly.ignore.desc': 'A backup or a sync is meant to send a lot.', 'network.anomaly.ignore.none': 'Every app is watched.',
    'sec.network.online': 'Connection notices',
    'network.watch.title': 'Tell me when the internet goes away', 'network.watch.desc': 'The notch says so when the internet is lost, when it is back, and when a network such as a hotel\'s asks you to sign in. Every five seconds it opens one small connection and, once, asks Apple\'s page for such checks. Works with the rest of this page off.',
    'network.login.title': 'Open the sign-in page', 'network.login.desc': 'Opens the page a network that asks for a login shows in the browser.', 'network.login.open': 'Open',
    'sec.network.dns': 'Encrypted DNS',
    'network.dns.title': 'Encrypt the names this Mac looks up', 'network.dns.desc': 'Websites and apps ask a DNS server for the address of every name, and anyone on the way can read those questions. This makes a profile that sends them encrypted (DNS over HTTPS) to the server you choose. macOS asks you to approve it in System Settings, General, Device Management, and you can remove it there at any time.',
    'network.dns.server': 'Server', 'network.dns.cloudflare': 'Cloudflare (1.1.1.1, fast)', 'network.dns.quad9': 'Quad9 (blocks known malicious sites)', 'network.dns.adguard': 'AdGuard (also blocks ads)', 'network.dns.google': 'Google Public DNS',
    'network.dns.nextdns': 'NextDNS (your own configuration)', 'network.dns.custom': 'Another server',
    'network.dns.nextdns.id': 'NextDNS configuration id', 'network.dns.nextdns.hint': 'The six letters and digits of your configuration, such as a1b2c3',
    'network.dns.custom.url': 'Server address', 'network.dns.custom.hint': 'https://dns.example.com/dns-query',
    'network.dns.install': 'Make the profile', 'network.dns.settings': 'Open Device Management',
    'network.dns.warn': 'On a company network or a VPN that has its own names, an outside server cannot look them up: remove the profile if work sites stop opening. Phone sharing asks the DNS of the Mac, so stop and start it again after changing this.',
    'sec.network.links': 'Connections',
    'network.links.desc': 'Each connection of this Mac, how well it answers, whether the web and secure sites open through it, and a test of its speed. A test takes a few seconds and moves up to 40 MB down and 12 MB up through that connection alone, so on a phone or a metered network leave it alone.',
    'network.speed.test': 'Test the connection', 'network.speed.running': 'Testing...', 'network.speed.again': 'Test again',
    'network.speed.done': 'Down {down}, up {up}, {rtt} ms to connect',
    'network.speed.done.noup': 'Down {down}, upload could not be measured, {rtt} ms to connect',
    'network.speed.plain': 'Measured without TLS, since secure sites do not open through this connection.',
    'network.speed.failed.offline': 'Nothing gets through this connection to the internet.',
    'network.speed.failed.login': 'This network asks for a sign-in before it lets anything through.',
    'network.speed.failed.tls': 'Secure connections through it were refused for their certificate, and the speed could not be measured.',
    'network.speed.failed.slow': 'It moved nothing in the time, so the speed could not be measured.',
    'network.speed.failed.failed': 'The test could not finish.',
    'network.web.none': 'The web does not open through this connection.',
    'network.web.login': 'This connection asks for a sign-in.',
    'network.web.tls': 'Secure sites (HTTPS) fail the certificate check here: the network may be opening secure traffic and signing it again with a certificate this Mac does not trust.',
    'network.web.fail': 'Secure sites (HTTPS) do not open through this connection.',
    'network.wifi.line': '{standard}, channel {channel} ({band} GHz, {width} MHz), signal {rssi} dBm, noise {noise} dBm, link {rate} Mbps',
    'network.wifi.grade.good': 'Strong signal', 'network.wifi.grade.fair': 'Fair signal', 'network.wifi.grade.poor': 'Weak signal',
    'sec.network.history': 'History',
    'network.history.title': 'Keep the history', 'network.history.desc': 'Adds up what each app sends and receives, by connection, for 90 days, also while this page is closed. Only the name of the app and the bytes are kept, on this Mac. An app that opens and closes a connection between two looks may be missed in part.',
    'network.history.pace.title': 'Look every', 'network.history.pace.desc': 'Shorter is more exact and costs a little more. While this page is open it looks every second anyway.', 'network.history.pace.n': '{n} seconds',
    'network.history.range.today': 'Today', 'network.history.range.week': '7 days', 'network.history.range.month': '30 days',
    'network.history.none': 'Nothing counted yet. Leave Keep the history on for a few minutes.', 'network.history.daily': 'Each day, last 14 days',
    'network.history.col.total': 'Total', 'network.history.unknown': 'mixed', 'network.history.all': 'All apps',
    'network.history.export.title': 'Export', 'network.history.export.desc': 'Writes a file into Downloads and shows it in Finder: one line for each day or month, app and connection.',
    'network.history.export.days': '{n} days', 'network.history.export.day': 'By day', 'network.history.export.month': 'By month', 'network.history.export.go': 'Export',
    'network.history.clear': 'Clear the history', 'network.history.clear.sure': 'Click again to clear', 'network.history.clear.desc': 'Forgets every day counted.',
    'sec.network.tether': 'Phone sharing (Beta)',
    'network.tether.title': 'Phone sharing', 'network.tether.desc': 'An Android phone on a USB cable uses this Mac\'s connection. Its Wi-Fi and mobile data can stay off.',
    'network.tether.missing': 'Phone sharing needs two free tools, installed once. Paste this in Terminal:',
    'network.tether.none': 'No phone connected', 'network.tether.none.hint': 'Plug in an Android phone with USB debugging on (Settings, Developer options).',
    'network.tether.unauthorized': 'Waiting for the phone', 'network.tether.unauthorized.hint': 'Unlock the phone and allow this Mac on its screen.',
    'network.tether.ready': 'Ready: {model}', 'network.tether.ready.hint': 'The phone asks once to allow a VPN connection: that is how it takes this Mac\'s connection.',
    'network.tether.running': 'Sharing with {model}', 'network.tether.running.hint': 'The phone is on this Mac\'s connection. Stop turns its VPN off too.',
    'network.tether.lost': 'Connection lost', 'network.tether.lost.hint': 'The phone was unplugged. Plug it in again to go on, or stop.',
    'network.tether.start': 'Start sharing', 'network.tether.stop': 'Stop sharing', 'network.tether.vpnoff': 'Turn off the phone\'s VPN',
    'network.tether.down': 'Down', 'network.tether.up': 'Up', 'network.tether.used': 'Used',
    'network.tether.test': 'Test the connection', 'network.tether.testing': 'Testing the phone\'s connection...',
    'network.tether.test.good': 'The phone reaches the internet and looks names up: {ms} ms.', 'network.tether.test.slow': 'The phone reaches the internet, but slowly: {ms} ms.',
    'network.tether.test.dns': 'The phone reaches the internet by address, but cannot look names up. Stop and start sharing again, so that it asks the DNS of this Mac.', 'network.tether.test.none': 'The phone does not reach the internet. Check that it allowed the VPN, or unplug and plug it in again.',
    'network.tether.test.untested': 'This phone has no tool to run the test. Open a website on it to check.',
    'network.tether.apps': 'The phone asks this Mac\'s DNS servers, so names resolve as they do here. A few apps only trust Wi-Fi or mobile data and may still say there is no connection.',
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
    'network.drag': 'Kéo từ viền vòng tròn của một đường mạng sang một app, hoặc từ viền ô app sang đường mạng. Nối Wi-Fi sang Ethernet để gộp: app nối vào vòng tròn lớn dùng cả hai, nối vào vòng tròn nhỏ chỉ dùng mạng đó. Kéo phần giữa ô hoặc vòng tròn để di chuyển. Giữ Space rồi kéo để di chuyển sơ đồ, giữ Control rồi cuộn để phóng to thu nhỏ.',
    'network.arrange': 'Sắp xếp', 'network.arrange.link': 'Theo đường mạng', 'network.arrange.traffic': 'Theo lưu lượng', 'network.arrange.name': 'Theo tên',
    'network.zoom.fit': 'Vừa khung', 'network.zoom.reset': 'Về 100%',
    'network.q.good': 'Mạng khoẻ: phản hồi trong {ms} ms', 'network.q.fair': 'Mạng tạm được: phản hồi trong {ms} ms, mất {loss}%',
    'network.q.poor': 'Mạng yếu: phản hồi trong {ms} ms, mất {loss}%', 'network.q.down': 'Không ra được internet qua đường này',
    'network.merge.split': 'Tách ra', 'network.merge.resize': 'Kéo để phóng to hoặc thu nhỏ vòng tròn', 'network.link.away': 'Chưa kết nối', 'network.app.idle': 'Hiện không dùng mạng', 'network.hub.merge.all': 'Gộp (mọi đường mạng)',
    'network.card.target': 'Đi qua', 'network.card.priority': 'Ưu tiên', 'network.card.down': 'Tải về tối đa (KB/s)', 'network.card.up': 'Tải lên tối đa (KB/s)', 'network.card.limit.none': 'Không giới hạn',
    'network.card.save': 'Lưu', 'network.card.unwire': 'Gỡ dây', 'network.card.held': 'Đang bị giữ ở {speed} để nhường app phía trên',
    'network.priority.high': 'Chính', 'network.priority.normal': 'Thường', 'network.priority.low': 'Xếp sau',
    'network.close': 'Đóng',
    'sec.network.metered': 'Tiết kiệm data',
    'network.metered.title': 'Tiết kiệm data trên đường mạng có giới hạn', 'network.metered.desc': 'Trên đường mạng tính tiền theo dung lượng, như mạng của điện thoại, chỉ các app bạn cho phép được dùng, và hạn mức có thể chặn phần còn lại. Chỉ áp dụng cho app theo cài đặt proxy, nên phải bật Áp dụng các dây; app khác chỉ được đếm.',
    'network.metered.auto.title': 'Coi mạng của điện thoại là có giới hạn', 'network.metered.auto.desc': 'Đường mạng tên iPhone hay Android, hoặc có địa chỉ mà điện thoại cấp khi chia sẻ dữ liệu, được coi là có giới hạn. Đây chỉ là phỏng đoán: đánh dấu bằng tay ở mục Đường mạng để sửa.',
    'network.metered.allow.title': 'App được dùng trên đường mạng có giới hạn', 'network.metered.allow.desc': 'Bản thân Figghost luôn được phép.', 'network.metered.allow.none': 'Chưa cho app nào: mọi app theo proxy đều bị giữ lại.',
    'network.metered.allow.add': 'Thêm app', 'network.metered.allow.remove': 'Bỏ {app}',
    'network.metered.cap.title': 'Hạn mức dữ liệu', 'network.metered.cap.desc': 'Lượng dữ liệu các đường mạng có giới hạn được dùng trong một kỳ, do Figghost đếm. Khi chạm hạn mức, mọi app theo proxy bị từ chối cho đến hết kỳ.', 'network.metered.cap.none': 'Không giới hạn',
    'network.metered.period.title': 'Tính theo', 'network.period.day': 'Ngày', 'network.period.month': 'Tháng',
    'network.metered.now': 'Đang dùng đường mạng có giới hạn ({link}): chỉ các app được phép chạy tiếp.', 'network.metered.not': 'Hiện không dùng đường mạng có giới hạn.',
    'network.metered.cap.used': 'Đã dùng trên các đường mạng có giới hạn trong kỳ này: {used} trên {cap}.', 'network.metered.cap.over': 'Đã chạm hạn mức: mọi app theo proxy bị từ chối.',
    'network.metered.mark': 'Có giới hạn', 'network.metered.mark.auto': 'Tự động', 'network.metered.mark.metered': 'Có giới hạn', 'network.metered.mark.not': 'Không giới hạn',
    'network.hub.block': 'Không đi đâu (chặn)', 'network.blocked': 'Đã chặn', 'network.refused': 'Đã từ chối {n}',
    'network.card.quota': 'Hạn mức dữ liệu (MB)', 'network.card.quota.period': 'Tính theo',
    'network.card.quota.used': 'Đã dùng {used} trên {quota} trong kỳ này',
    'network.card.hosts': 'Trang web và địa chỉ nó kết nối tới', 'network.card.hosts.none': 'Chưa thấy gì. App theo proxy hiện tên trang, app khác hiện địa chỉ.',
    'network.card.block.note': 'Đã chặn: mọi kết nối app này mở qua proxy đều bị từ chối.',
    'sec.network.anomaly': 'Tải lên đột biến',
    'network.anomaly.title': 'Báo khi một app tải lên rất nhiều', 'network.anomaly.desc': 'Notch báo khi một app gửi đi trong năm phút nhiều hơn 50 MB và nhiều hơn mười lần lượng nó thường gửi trong một giờ. Cần app đã có dữ liệu một ngày, Figghost đếm sẵn cho nó.',
    'network.anomaly.ignore.title': 'App không bao giờ báo', 'network.anomaly.ignore.desc': 'Sao lưu hay đồng bộ thì vốn gửi nhiều.', 'network.anomaly.ignore.none': 'Mọi app đều được theo dõi.',
    'sec.network.online': 'Thông báo kết nối',
    'network.watch.title': 'Báo khi mất internet', 'network.watch.desc': 'Notch báo khi mất internet, khi có lại, và khi một mạng như Wi-Fi khách sạn yêu cầu đăng nhập. Cứ năm giây mở một kết nối nhỏ và hỏi trang kiểm tra của Apple. Hoạt động cả khi phần còn lại của trang này tắt.',
    'network.login.title': 'Mở trang đăng nhập', 'network.login.desc': 'Mở trên trình duyệt trang mà mạng yêu cầu đăng nhập hiển thị.', 'network.login.open': 'Mở',
    'sec.network.dns': 'DNS mã hóa',
    'network.dns.title': 'Mã hóa các tên miền máy Mac tra cứu', 'network.dns.desc': 'Trang web và app hỏi máy chủ DNS địa chỉ của từng tên miền, và bất kỳ ai trên đường đi đều đọc được các câu hỏi đó. Mục này tạo một hồ sơ gửi chúng đã mã hóa (DNS over HTTPS) tới máy chủ bạn chọn. macOS sẽ yêu cầu bạn duyệt trong Cài đặt hệ thống, Cài đặt chung, Quản lý thiết bị, và bạn gỡ được ở đó bất cứ lúc nào.',
    'network.dns.server': 'Máy chủ', 'network.dns.cloudflare': 'Cloudflare (1.1.1.1, nhanh)', 'network.dns.quad9': 'Quad9 (chặn các trang độc hại đã biết)', 'network.dns.adguard': 'AdGuard (chặn cả quảng cáo)', 'network.dns.google': 'Google Public DNS',
    'network.dns.nextdns': 'NextDNS (cấu hình riêng của bạn)', 'network.dns.custom': 'Máy chủ khác',
    'network.dns.nextdns.id': 'Mã cấu hình NextDNS', 'network.dns.nextdns.hint': 'Sáu chữ và số của cấu hình, như a1b2c3',
    'network.dns.custom.url': 'Địa chỉ máy chủ', 'network.dns.custom.hint': 'https://dns.example.com/dns-query',
    'network.dns.install': 'Tạo hồ sơ', 'network.dns.settings': 'Mở Quản lý thiết bị',
    'network.dns.warn': 'Trên mạng công ty hoặc VPN có tên miền riêng, máy chủ bên ngoài không tra được chúng: hãy gỡ hồ sơ nếu các trang công việc không mở được. Phone sharing hỏi DNS của máy Mac, nên hãy dừng rồi bật lại sau khi đổi mục này.',
    'sec.network.links': 'Đường mạng',
    'network.links.desc': 'Từng đường mạng của máy Mac này, mức phản hồi của nó, web thường và web bảo mật có mở được qua nó không, và phép đo tốc độ. Mỗi lần đo mất vài giây và chuyển tối đa 40 MB xuống và 12 MB lên chỉ qua đường mạng đó, nên đừng đo trên mạng điện thoại hay mạng có giới hạn.',
    'network.speed.test': 'Kiểm tra kết nối', 'network.speed.running': 'Đang kiểm tra...', 'network.speed.again': 'Kiểm tra lại',
    'network.speed.done': 'Tải xuống {down}, tải lên {up}, {rtt} ms để kết nối',
    'network.speed.done.noup': 'Tải xuống {down}, không đo được tải lên, {rtt} ms để kết nối',
    'network.speed.plain': 'Đo không qua TLS, vì các trang bảo mật không mở được qua đường mạng này.',
    'network.speed.failed.offline': 'Không có gì ra được internet qua đường mạng này.',
    'network.speed.failed.login': 'Mạng này yêu cầu đăng nhập trước khi cho gì đi qua.',
    'network.speed.failed.tls': 'Kết nối bảo mật qua nó bị từ chối vì chứng chỉ, nên không đo được tốc độ.',
    'network.speed.failed.slow': 'Nó không chuyển được gì trong thời gian cho phép, nên không đo được tốc độ.',
    'network.speed.failed.failed': 'Phép kiểm tra không hoàn thành.',
    'network.web.none': 'Web không mở được qua đường mạng này.',
    'network.web.login': 'Đường mạng này yêu cầu đăng nhập.',
    'network.web.tls': 'Các trang bảo mật (HTTPS) không qua được bước kiểm tra chứng chỉ ở đây: mạng có thể đang mở gói tin bảo mật rồi ký lại bằng một chứng chỉ mà máy Mac này không tin.',
    'network.web.fail': 'Các trang bảo mật (HTTPS) không mở được qua đường mạng này.',
    'network.wifi.line': '{standard}, kênh {channel} ({band} GHz, {width} MHz), tín hiệu {rssi} dBm, nhiễu {noise} dBm, liên kết {rate} Mbps',
    'network.wifi.grade.good': 'Sóng mạnh', 'network.wifi.grade.fair': 'Sóng vừa', 'network.wifi.grade.poor': 'Sóng yếu',
    'sec.network.history': 'Lịch sử',
    'network.history.title': 'Lưu lịch sử', 'network.history.desc': 'Cộng dồn lượng dữ liệu mỗi app gửi và nhận, theo từng đường mạng, trong 90 ngày, cả khi trang này đóng. Chỉ giữ tên app và số byte, trên máy này. App mở rồi đóng kết nối giữa hai lần đọc có thể bị tính thiếu một phần.',
    'network.history.pace.title': 'Đọc mỗi', 'network.history.pace.desc': 'Ngắn hơn thì chính xác hơn và tốn thêm một chút. Khi trang này đang mở thì vẫn đọc mỗi giây.', 'network.history.pace.n': '{n} giây',
    'network.history.range.today': 'Hôm nay', 'network.history.range.week': '7 ngày', 'network.history.range.month': '30 ngày',
    'network.history.none': 'Chưa tính được gì. Hãy để Lưu lịch sử bật vài phút.', 'network.history.daily': 'Từng ngày, 14 ngày gần nhất',
    'network.history.col.total': 'Tổng', 'network.history.unknown': 'nhiều đường', 'network.history.all': 'Mọi app',
    'network.history.export.title': 'Xuất dữ liệu', 'network.history.export.desc': 'Ghi một tệp vào thư mục Downloads và hiện nó trong Finder: mỗi dòng là một ngày hoặc một tháng, một app, một đường mạng.',
    'network.history.export.days': '{n} ngày', 'network.history.export.day': 'Theo ngày', 'network.history.export.month': 'Theo tháng', 'network.history.export.go': 'Xuất',
    'network.history.clear': 'Xóa lịch sử', 'network.history.clear.sure': 'Bấm lần nữa để xóa', 'network.history.clear.desc': 'Quên hết các ngày đã tính.',
    'sec.network.tether': 'Phone sharing (Beta)',
    'network.tether.title': 'Phone sharing', 'network.tether.desc': 'Điện thoại Android cắm cáp USB dùng mạng của máy Mac này. Wi-Fi và dữ liệu di động của điện thoại có thể tắt.',
    'network.tether.missing': 'Phone sharing cần hai công cụ miễn phí, chỉ cài một lần. Dán lệnh này vào Terminal:',
    'network.tether.none': 'Chưa có điện thoại', 'network.tether.none.hint': 'Cắm điện thoại Android đã bật Gỡ lỗi USB (Cài đặt, Tùy chọn nhà phát triển).',
    'network.tether.unauthorized': 'Đang chờ điện thoại', 'network.tether.unauthorized.hint': 'Mở khóa điện thoại và bấm Cho phép máy Mac này trên màn hình.',
    'network.tether.ready': 'Sẵn sàng: {model}', 'network.tether.ready.hint': 'Lần đầu điện thoại hỏi cho phép kết nối VPN: đó là cách nó dùng mạng của máy Mac.',
    'network.tether.running': 'Đang chia mạng cho {model}', 'network.tether.running.hint': 'Điện thoại đang dùng mạng của máy Mac. Bấm Dừng sẽ tắt luôn VPN trên điện thoại.',
    'network.tether.lost': 'Mất kết nối', 'network.tether.lost.hint': 'Điện thoại đã bị rút cáp. Cắm lại để tiếp tục, hoặc bấm Dừng.',
    'network.tether.start': 'Bắt đầu chia mạng', 'network.tether.stop': 'Dừng chia mạng', 'network.tether.vpnoff': 'Tắt VPN trên điện thoại',
    'network.tether.down': 'Tải về', 'network.tether.up': 'Tải lên', 'network.tether.used': 'Đã dùng',
    'network.tether.test': 'Kiểm tra kết nối', 'network.tether.testing': 'Đang kiểm tra kết nối của điện thoại...',
    'network.tether.test.good': 'Điện thoại vào được internet và phân giải được tên miền: {ms} ms.', 'network.tether.test.slow': 'Điện thoại vào được internet nhưng chậm: {ms} ms.',
    'network.tether.test.dns': 'Điện thoại ra được internet bằng địa chỉ IP nhưng không phân giải được tên miền. Hãy dừng rồi bắt đầu chia mạng lại để điện thoại dùng DNS của máy Mac.', 'network.tether.test.none': 'Điện thoại không vào được internet. Kiểm tra điện thoại đã cho phép VPN chưa, hoặc rút cáp rồi cắm lại.',
    'network.tether.test.untested': 'Điện thoại này không có công cụ để kiểm tra. Mở một trang web trên điện thoại để thử.',
    'network.tether.apps': 'Điện thoại hỏi tên miền qua DNS của máy Mac, nên phân giải giống hệt trên máy này. Một vài app chỉ tin Wi-Fi hoặc dữ liệu di động và vẫn có thể báo không có mạng.',
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


    // ---- encrypted DNS: the choice, what it needs, and the buttons that make the profile and open its place
    const dnsOptions = () => ['cloudflare', 'quad9', 'adguard', 'google', 'nextdns', 'custom'].map((v) => ({ value: v, label: t('network.dns.' + v) }));
    const dnsText = (path, label, hint, shown) => {
      const input = h('input', { type: 'text', class: 'text-field network-dns-input', placeholder: hint, 'aria-label': label, spellcheck: 'false', autocomplete: 'off' });
      input.addEventListener('change', () => change(path, input.value.trim()));
      const el = row(label, null, input);
      reg(() => { if (document.activeElement !== input) input.value = get(path) || ''; el.hidden = !shown(); });
      return el;
    };
    const dnsNext = dnsText('network.dns_nextdns', t('network.dns.nextdns.id'), t('network.dns.nextdns.hint'), () => N().dns === 'nextdns');
    const dnsCustom = dnsText('network.dns_custom', t('network.dns.custom.url'), t('network.dns.custom.hint'), () => N().dns === 'custom');
    const dnsInstall = button(t('network.dns.install'), () => act('network_dns_install'), { primary: true, small: true });
    reg(() => {
      const d = N().dns;
      dnsInstall.disabled = (d === 'nextdns' && !N().dns_nextdns) || (d === 'custom' && !N().dns_custom);
    });

    // ---- save data: what is allowed on a metered connection, and the cap
    const sizeMb = (mb) => (mb >= 1000 ? (mb / 1000) + ' GB' : mb + ' MB');
    const capOptions = () => [0, 500, 1000, 2000, 5000, 10000, 20000, 50000].map((v) => ({ value: v, label: v ? sizeMb(v) : t('network.metered.cap.none') }));
    const periodOptions = () => ['day', 'month'].map((v) => ({ value: v, label: t('network.period.' + v) }));
    // A list of app names kept as chips, with a menu of the apps seen to add one: the allowed apps, the apps left alone.
    const chipEditor = ({ path, list, none, add, remove }) => {
      const chips = h('div', { class: 'network-chips' });
      const sel = h('select', { class: 'pick', 'aria-label': add });
      const write = (names) => send({ t: 'set', path, value: names });
      const addBtn = button(add, () => { if (sel.value) write([...list(), sel.value]); }, { small: true });
      reg(() => {
        const names = list();
        chips.replaceChildren(...(names.length
          ? names.map((name) => h('span', { class: 'network-chip' }, name,
            h('button', { class: 'network-chip-x', 'aria-label': remove.replace('{app}', name), onclick: () => write(names.filter((n) => n !== name)) }, '\u00d7')))
          : [h('span', { class: 'row-desc', text: none })]));
        const seen = [...new Set([...(N().known_apps || []).map((a) => a.name), ...(N().apps || []).map((a) => a.name)])]
          .filter((n) => !names.some((x) => x.toLowerCase() === n.toLowerCase())).sort((a, b) => a.localeCompare(b));
        const key = seen.join('|');
        if (sel.dataset.key !== key) {
          sel.dataset.key = key;
          sel.replaceChildren(...seen.map((n) => h('option', { value: n, text: n })));
        }
        addBtn.disabled = !seen.length;
      });
      return { chips, controls: h('div', { class: 'network-allow' }, sel, addBtn) };
    };
    const allowEd = chipEditor({ path: 'network.metered.allow', list: () => N().metered.allow || [], none: t('network.metered.allow.none'), add: t('network.metered.allow.add'), remove: t('network.metered.allow.remove', { app: '{app}' }) });
    const ignoreEd = chipEditor({ path: 'network.anomaly_ignore', list: () => N().anomaly_ignore || [], none: t('network.anomaly.ignore.none'), add: t('network.metered.allow.add'), remove: t('network.metered.allow.remove', { app: '{app}' }) });
    const meteredStatus = h('div', { class: 'row-desc network-status' });
    reg(() => {
      const p = N().policy || {};
      const m = N().metered || {};
      const cap = (m.cap_mb || 0) * 1e6;
      const lines = [p.metered_now ? t('network.metered.now', { link: p.link || '' }) : t('network.metered.not')];
      if (cap) lines.push(t('network.metered.cap.used', { used: size(p.cap_used || 0), cap: size(cap) }));
      if (p.cap_level === 'over') lines.push(t('network.metered.cap.over'));
      meteredStatus.textContent = lines.join(' ');
      meteredStatus.dataset.state = p.cap_level === 'over' ? 'failed' : p.metered_now ? 'on' : 'off';
    });
    const meteredRow = (el) => { reg(() => el.classList.toggle('disabled', !(on() && N().metered.enabled))); return el; };

    // ---- connections: each link with how well it answers, the radio of the Wi-Fi one, and a test of its speed
    const mbps = (bps) => {
      const n = Number(bps) || 0;
      return n >= 1e9 ? (n / 1e9).toFixed(2) + ' Gbps' : (n / 1e6).toFixed(1) + ' Mbps';
    };
    const linkRows = new Map();
    const linksBox = h('div', { class: 'network-linklist' });
    const makeLinkRow = (name) => {
      const title = h('div', { class: 'row-title' });
      const line = h('div', { class: 'row-desc' });
      const wifiLine = h('div', { class: 'row-desc network-wifi' });
      const webLine = h('div', { class: 'row-desc network-web' });
      const result = h('div', { class: 'row-desc network-speed-result' });
      const btn = button(t('network.speed.test'), () => act('network_speed_test:' + name), { small: true });
      const mark = h('select', { class: 'pick network-mark', 'aria-label': t('network.metered.mark') },
        ...['auto', 'metered', 'not'].map((v) => h('option', { value: v, text: t('network.metered.mark.' + v) })));
      mark.addEventListener('change', () => send({ t: 'set', path: 'network.metered.mark', value: { name, state: mark.value } }));
      const el = h('div', { class: 'row' }, h('div', { class: 'row-text' }, title, line, wifiLine, webLine, result), h('div', { class: 'row-control network-link-control' }, mark, btn));
      return { el, title, line, wifiLine, webLine, result, btn, mark };
    };
    reg(() => {
      const links = (N().links || []).filter((l) => l.name);
      const names = links.map((l) => l.name);
      for (const [name, r] of linkRows) if (!names.includes(name)) { r.el.remove(); linkRows.delete(name); }
      for (const l of links) {
        let r = linkRows.get(l.name);
        if (!r) { r = makeLinkRow(l.name); linkRows.set(l.name, r); }
        linksBox.append(r.el);
        r.title.textContent = l.label + ' (' + l.name + ')';
        const q = l.quality && l.quality !== 'unknown' ? (l.quality === 'down' ? t('network.q.down') : t('network.q.' + l.quality, { ms: l.rtt, loss: Math.round((l.loss || 0) * 100) })) : '';
        r.line.textContent = [l.ip, q].filter(Boolean).join(' - ');
        const w = l.wifi;
        r.wifiLine.hidden = !w;
        if (w) r.wifiLine.textContent = t('network.wifi.grade.' + w.grade) + ': ' + t('network.wifi.line', { standard: w.standard || '802.11', channel: w.channel, band: w.band, width: w.width, rssi: w.rssi, noise: w.noise, rate: w.rate });
        // What is wrong with the web through the connection, found by asking it and not by the answer of a handshake.
        const web = l.web || (l.speed_test && l.speed_test.web ? { http: l.speed_test.web, https: l.speed_test.https } : null);
        const problem = !web ? '' : web.http === 'none' ? 'none' : web.http === 'login' ? 'login' : web.https === 'tls' ? 'tls' : web.https === 'fail' ? 'fail' : '';
        r.webLine.hidden = !problem;
        r.webLine.textContent = problem ? t('network.web.' + problem) : '';
        r.webLine.dataset.state = problem ? 'failed' : '';
        const m = N().metered || {};
        r.mark.value = (m.marked || []).includes(l.name) ? 'metered' : (m.unmarked || []).includes(l.name) ? 'not' : 'auto';
        r.mark.hidden = !m.enabled;
        const test = l.speed_test;
        r.btn.textContent = test && test.state === 'running' ? t('network.speed.running') : test ? t('network.speed.again') : t('network.speed.test');
        r.btn.disabled = !!(test && test.state === 'running');
        r.result.hidden = !test || test.state === 'running';
        if (test && test.state === 'done') {
          const text = test.up == null
            ? t('network.speed.done.noup', { down: mbps(test.down), rtt: test.rtt })
            : t('network.speed.done', { down: mbps(test.down), up: mbps(test.up), rtt: test.rtt });
          r.result.textContent = test.plain ? text + ' ' + t('network.speed.plain') : text;
        } else if (test && test.state === 'failed') {
          r.result.textContent = t('network.speed.failed.' + test.reason);
        }
        r.result.dataset.state = test ? test.state : '';
      }
      if (!links.length) linksBox.replaceChildren(h('div', { class: 'row-desc network-empty', text: t('network.links.none') }));
      else linksBox.querySelector('.network-empty')?.remove();
    });

    // ---- history: what each app moved over the days, with the choice of range, the bars of each day and the way out
    const range = { v: 'today' };
    const exp = { format: 'csv', days: 7, grain: 'day' };
    const rangeSeg = segmented({
      label: t('sec.network.history'),
      options: ['today', 'week', 'month'].map((k) => ({ value: k, label: t('network.history.range.' + k) })),
      value: () => range.v,
      onPick: (v) => { range.v = v; runSyncs(); },
    });
    const histTable = h('div', { class: 'network-table network-history-table', role: 'table' });
    const bars = h('div', { class: 'network-bars', role: 'img' });
    const barsTitle = h('div', { class: 'row-desc', text: t('network.history.daily') });
    const viaOf = (a) => (a.interfaces || []).map((i) => (i.name === '?' ? t('network.history.unknown') : linkName(i.name))).join(', ') || '-';
    reg(() => {
      const view = N().history_view || {};
      const list = view[range.v] || [];
      if (!list.length) {
        histTable.replaceChildren(h('div', { class: 'row-desc network-empty', text: t('network.history.none') }));
      } else {
        const head = h('div', { class: 'network-tr head', role: 'row' },
          ...['app', 'down', 'up', 'total', 'via'].map((k) => h('span', { role: 'columnheader', text: t(k === 'total' ? 'network.history.col.total' : 'network.col.' + k) })));
        histTable.replaceChildren(head, ...list.map((a) => h('div', { class: 'network-tr', role: 'row' },
          h('span', { class: 'network-app', text: a.app }),
          h('span', { text: size(a.down) }),
          h('span', { text: size(a.up) }),
          h('span', { text: size(a.down + a.up) }),
          h('span', { class: 'network-via', text: viaOf(a) }))));
      }
      const daily = view.daily || [];
      const most = Math.max(1, ...daily.map((d) => d.down + d.up));
      bars.replaceChildren(...daily.map((d) => {
        const bar = h('span', { class: 'network-bar-fill', style: `height:${Math.max(2, Math.round((d.down + d.up) / most * 100))}%` });
        return h('span', { class: 'network-bar', title: `${d.day}: ${size(d.down)} / ${size(d.up)}` }, bar, h('span', { class: 'network-bar-day', text: d.day.slice(0, 2) }));
      }));
      bars.setAttribute('aria-label', t('network.history.daily'));
      bars.hidden = barsTitle.hidden = view.empty !== false;
    });
    const pick = (opts, current, onChange, label) => {
      const el = h('select', { class: 'pick', 'aria-label': label }, ...opts.map((o) => h('option', { value: String(o.value), text: o.label })));
      el.value = String(current);
      el.addEventListener('change', () => onChange(el.value));
      return el;
    };
    const exportControls = h('div', { class: 'network-export' },
      pick([{ value: 'csv', label: 'CSV' }, { value: 'json', label: 'JSON' }], exp.format, (v) => { exp.format = v; }, 'format'),
      pick([7, 30, 90].map((n) => ({ value: n, label: t('network.history.export.days', { n }) })), exp.days, (v) => { exp.days = +v; }, 'days'),
      pick(['day', 'month'].map((k) => ({ value: k, label: t('network.history.export.' + k) })), exp.grain, (v) => { exp.grain = v; }, 'grain'),
      button(t('network.history.export.go'), () => act(`network_history_export:${exp.format}:${exp.days}:${exp.grain}`), { small: true, primary: true }));
    let armed = false, armTimer = 0;
    const wipe = button(t('network.history.clear'), () => {
      if (!armed) {
        armed = true;
        wipe.textContent = t('network.history.clear.sure');
        armTimer = setTimeout(() => { armed = false; wipe.textContent = t('network.history.clear'); }, 4000);
        return;
      }
      clearTimeout(armTimer);
      armed = false;
      wipe.textContent = t('network.history.clear');
      act('network_history_clear');
    }, { small: true, danger: true });
    const paceOptions = () => [2, 5, 10].map((n) => ({ value: n, label: t('network.history.pace.n', { n }) }));

    // ---- wiring: the switch, where it stands, the way back
    const status = h('span', { class: 'row-desc network-status' });
    reg(() => {
      const n = N();
      status.textContent = n.paused && n.running ? t('network.paused') : t('network.status.' + (n.status || 'off'));
      status.dataset.state = n.status || 'off';
    });
    const restoreRow = row(t('network.restore'), t('network.restore.desc'), button(t('network.restore'), () => act('network_restore'), { small: true }));
    reg(() => { restoreRow.hidden = !N().saved || (N().control && N().status === 'on'); });

    // ---- Phone sharing: one card that says where it stands with a coloured light, and the one button that moves on.
    const light = h('span', { class: 'phone-light', 'aria-hidden': 'true' });
    const badge = h('div', { class: 'phone-badge' }, icon('iphone.radiowaves.left.and.right', 'phone-glyph'), light);
    const phoneTitle = h('div', { class: 'phone-state' });
    const phoneHint = h('div', { class: 'phone-hint' });
    // The command is written only while it is needed: the card does not name the tool it runs otherwise.
    const install = h('code', { class: 'code phone-install' });
    const start = button(t('network.tether.start'), () => act('network_tether_start'), { small: true, primary: true });
    const stop = button(t('network.tether.stop'), () => act('network_tether_stop'), { small: true, danger: true });
    const vpnOff = button(t('network.tether.vpnoff'), () => act('network_tether_stop'), { small: true });
    // The meter: what the phone moves through the Mac, with a bar each that follows the speed.
    const gauge = (label) => {
      const value = h('span', { class: 'phone-gauge-value' });
      const fill = h('span', { class: 'phone-gauge-fill' });
      return { el: h('div', { class: 'phone-gauge' }, h('span', { class: 'phone-gauge-label', text: label }), value, h('span', { class: 'phone-gauge-bar' }, fill)), value, fill };
    };
    const gDown = gauge(t('network.tether.down')), gUp = gauge(t('network.tether.up'));
    const used = h('div', { class: 'phone-used' });
    const meter = h('div', { class: 'phone-meter' }, gDown.el, gUp.el, used);
    const testBtn = button(t('network.tether.test'), () => act('network_tether_test'), { small: true });
    const testLine = h('div', { class: 'phone-test' });
    const phoneCard = h('div', { class: 'phone-card' },
      badge,
      h('div', { class: 'phone-text' }, h('div', { class: 'phone-name', text: t('network.tether.title') }), phoneTitle, phoneHint, install),
      h('div', { class: 'phone-actions' }, start, vpnOff, stop, testBtn),
      meter, testLine);
    reg(() => {
      const tt = N().tether || {};
      const tools = tt.gnirehtet && tt.adb;
      const phones = tt.phones || [];
      const model = phones.map((p) => p.model || p.serial).join(', ');
      const state = !tools ? 'missing' : tt.running ? (phones.length ? 'running' : 'lost') : tt.unauthorized ? 'unauthorized' : phones.length ? 'ready' : 'none';
      phoneCard.dataset.state = state;
      phoneTitle.textContent = state === 'missing' ? '' : t('network.tether.' + state, { model });
      phoneHint.textContent = state === 'missing' ? t('network.tether.missing') : t('network.tether.' + state + '.hint');
      install.hidden = state !== 'missing';
      install.textContent = state === 'missing' ? 'brew install gnirehtet android-platform-tools' : '';
      phoneTitle.hidden = state === 'missing';
      // A button's own display wins over `hidden`, so they are shown and hidden by their style.
      const show = (el, yes) => { el.style.display = yes ? '' : 'none'; };
      show(start, state === 'ready');
      show(stop, !!tt.running);
      show(vpnOff, state === 'ready');
      const test = tt.test || { state: 'idle' };
      show(testBtn, (state === 'ready' || state === 'running') && test.state !== 'running');
      // A full bar is 20 MB/s, on a scale of tens, so that a slow phone still moves its bar.
      const level = (b) => Math.min(100, Math.max(0, Math.log10(1 + (Number(b) || 0) / 1e3) / Math.log10(2e4) * 100));
      const m = tt.meter || {};
      meter.hidden = state !== 'running';
      gDown.value.textContent = speed(m.down); gDown.fill.style.width = level(m.down) + '%';
      gUp.value.textContent = speed(m.up); gUp.fill.style.width = level(m.up) + '%';
      used.textContent = t('network.tether.used') + ': ' + size((m.received || 0) + (m.sent || 0));
      testLine.dataset.verdict = test.state === 'done' ? test.verdict : test.state;
      testLine.textContent = test.state === 'running' ? t('network.tether.testing')
        : test.state === 'done' ? (test.phone ? test.phone + ': ' : '') + t('network.tether.test.' + test.verdict, { ms: test.ms == null ? '-' : test.ms, loss: test.loss })
        : '';
      testLine.hidden = !testLine.textContent;
    });

    return [
      card(toggleRow('network.enabled', t('network.enable.title'), t('network.enable.desc')),
        row(t('network.open'), t('network.open.desc'), button(t('network.open'), () => act('network_canvas_open'), { primary: true, small: true }), when)),
      sectionTitle(t('sec.network.summary')),
      card(row(total, null, null, when), h('div', { class: 'row block network-table-row' }, table)),
      sectionTitle(t('sec.network.metered')),
      card(toggleRow('network.metered.enabled', t('network.metered.title'), t('network.metered.desc'), when),
        meteredRow(toggleRow('network.metered.auto', t('network.metered.auto.title'), t('network.metered.auto.desc'))),
        meteredRow(row(t('network.metered.allow.title'), t('network.metered.allow.desc'), allowEd.controls)),
        meteredRow(h('div', { class: 'row block' }, allowEd.chips)),
        selectRow({ path: 'network.metered.cap_mb', title: t('network.metered.cap.title'), desc: t('network.metered.cap.desc'), options: capOptions, when: () => on() && N().metered.enabled }),
        selectRow({ path: 'network.metered.period', title: t('network.metered.period.title'), desc: null, options: periodOptions, when: () => on() && N().metered.enabled && N().metered.cap_mb > 0 }),
        row(meteredStatus, null, null, when)),
      sectionTitle(t('sec.network.online')),
      card(toggleRow('network.watch', t('network.watch.title'), t('network.watch.desc')),
        row(t('network.login.title'), t('network.login.desc'), button(t('network.login.open'), () => act('network_open_login'), { small: true }))),
      sectionTitle(t('sec.network.dns')),
      card(row(t('network.dns.title'), t('network.dns.desc'), null),
        selectRow({ path: 'network.dns', title: t('network.dns.server'), desc: null, options: dnsOptions }),
        dnsNext, dnsCustom,
        row(t('network.dns.install'), t('network.dns.warn'), h('div', { class: 'network-allow' }, dnsInstall, button(t('network.dns.settings'), () => act('network_dns_settings'), { small: true })))),
      sectionTitle(t('sec.network.anomaly')),
      card(toggleRow('network.anomaly', t('network.anomaly.title'), t('network.anomaly.desc'), when),
        row(t('network.anomaly.ignore.title'), t('network.anomaly.ignore.desc'), ignoreEd.controls, { when: () => on() && N().anomaly }),
        h('div', { class: 'row block' }, ignoreEd.chips)),
      sectionTitle(t('sec.network.links')),
      card(h('div', { class: 'row block' }, linksBox), h('p', { class: 'network-note', text: t('network.links.desc') })),
      sectionTitle(t('sec.network.history')),
      card(toggleRow('network.history', t('network.history.title'), t('network.history.desc'), when),
        selectRow({ path: 'network.history_seconds', title: t('network.history.pace.title'), desc: t('network.history.pace.desc'), options: paceOptions, when: () => on() && N().history }),
        row(rangeSeg, null, null, when),
        h('div', { class: 'row block network-table-row' }, histTable),
        h('div', { class: 'row block network-bars-row' }, barsTitle, bars),
        row(t('network.history.export.title'), t('network.history.export.desc'), exportControls, when),
        row(t('network.history.clear'), t('network.history.clear.desc'), wipe, when)),
      sectionTitle(t('sec.network.control')),
      card(toggleRow('network.control', t('network.control.title'), t('network.control.desc'), when),
        row(status, null, null, when), restoreRow),
      sectionTitle(t('sec.network.tether')),
      card(h('div', { class: 'row block' }, phoneCard),
        h('p', { class: 'network-note', text: t('network.tether.desc') + ' ' + t('network.tether.apps') + ' ' + t('network.tether.iphone') })),
    ];
  },

  // The diagram, in a window of its own (the page told `__canvas` is network): connections as circles, apps as boxes, all
  // moved freely on a dotted grid; one connection wired to another makes the Combined circle around both.
  canvas() {
    const N = () => S.network;
    const send1 = (path, value) => send({ t: 'set', path, value });
    const { speed, size, linkName } = netHelpers();
    // D: a connection on its own; C: one inside the Combined circle, smaller so that it can move about in there.
    const GRID = 20, D = 96, C = 70, APP_W = 184, APP_H = 54;
    const MERGE_MIN = 200, MERGE_MAX = 640;
    // A connection inside the circle keeps its middle this far within the circle's edge.
    const inCircle = (rel, size) => {
      const r = size / 2 - C / 2 - 8;
      const cx = rel.x + C / 2 - size / 2, cy = rel.y + C / 2 - size / 2;
      const d = Math.hypot(cx, cy);
      const k = d > r ? r / d : 1;
      return { x: Math.round(size / 2 + cx * k - C / 2), y: Math.round(size / 2 + cy * k - C / 2) };
    };
    const snap = (v) => Math.max(0, Math.round(v / GRID) * GRID);

    const head = h('header', { class: 'network-canvas-head', 'data-drag': '' });
    const title = h('h1', { text: t('page.network') });
    const totals = h('span', { class: 'network-total-head' });
    const note = h('span', { class: 'network-canvas-note' });
    const close = button(t('network.close'), () => send({ t: 'action', name: 'network_canvas_close' }), { small: true });
    // Arrange is a one-shot: the choice lays the boxes out, then the menu goes back to its title.
    const arrangeSel = h('select', { class: 'select network-arrange', 'aria-label': t('network.arrange') },
      h('option', { value: '', text: t('network.arrange') }),
      ...['link', 'traffic', 'name'].map((k) => h('option', { value: k, text: t('network.arrange.' + k) })));
    arrangeSel.addEventListener('change', () => { if (arrangeSel.value) arrange(arrangeSel.value); arrangeSel.value = ''; });
    const zoomBtn = button('100%', () => zoomTo(1), { small: true });
    zoomBtn.title = t('network.zoom.reset');
    const fitBtn = button(t('network.zoom.fit'), () => fit(), { small: true });
    head.append(title, totals, note, arrangeSel, fitBtn, zoomBtn, close);
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

    // The view: the board is moved and scaled inside the window; the dotted grid on the window follows it.
    const view = { x: 0, y: 0, z: 1 };
    const Z_MIN = 0.3, Z_MAX = 2.5;
    function applyView() {
      board.style.transform = `translate(${view.x}px, ${view.y}px) scale(${view.z})`;
      scroller.style.backgroundSize = `${GRID * view.z}px ${GRID * view.z}px`;
      scroller.style.backgroundPosition = `${view.x}px ${view.y}px`;
      zoomBtn.textContent = Math.round(view.z * 100) + '%';
    }
    // Zooms about a point of the window, so what is under the pointer stays under it.
    function zoomAt(z, px, py) {
      const nz = Math.min(Z_MAX, Math.max(Z_MIN, z));
      view.x = px - (px - view.x) * nz / view.z;
      view.y = py - (py - view.y) * nz / view.z;
      view.z = nz;
      applyView();
    }
    function zoomTo(z) { zoomAt(z, scroller.clientWidth / 2, scroller.clientHeight / 2); }
    // Everything on the board in sight, never larger than 100%.
    function fit() {
      if (!current) return;
      let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
      for (const [id, p] of Object.entries(current.at)) {
        const w = p.size || p.d || (id.startsWith('app:') ? APP_W : D), hh = p.size || p.d || (id.startsWith('app:') ? APP_H : D);
        x0 = Math.min(x0, p.x); y0 = Math.min(y0, p.y); x1 = Math.max(x1, p.x + w); y1 = Math.max(y1, p.y + hh);
      }
      if (!isFinite(x0)) return;
      const W = scroller.clientWidth || 900, H = scroller.clientHeight || 600, pad = 40;
      const z = Math.min(1, Math.max(Z_MIN, Math.min((W - pad * 2) / (x1 - x0), (H - pad * 2) / (y1 - y0))));
      view.z = z;
      view.x = (W - (x1 - x0) * z) / 2 - x0 * z;
      view.y = (H - (y1 - y0) * z) / 2 - y0 * z;
      applyView();
    }
    // Control and the wheel (a pinch on a trackpad comes as the same) zooms; the wheel alone moves the view.
    scroller.addEventListener('wheel', (e) => {
      e.preventDefault();
      const b = scroller.getBoundingClientRect();
      if (e.ctrlKey || e.metaKey) zoomAt(view.z * Math.exp(-e.deltaY * 0.01), e.clientX - b.left, e.clientY - b.top);
      else { view.x -= e.deltaX; view.y -= e.deltaY; applyView(); }
    }, { passive: false });
    // Space held, the middle button, or a drag on the empty board moves the view; nodes are left alone meanwhile.
    let spaceDown = false;
    scroller.addEventListener('pointerdown', (e) => {
      const onNode = e.target.closest && e.target.closest('[data-node]');
      if (!(spaceDown || e.button === 1 || (e.button === 0 && !onNode))) return;
      e.preventDefault();
      e.stopPropagation();
      const start = { x: e.clientX - view.x, y: e.clientY - view.y };
      scroller.classList.add('panning');
      scroller.setPointerCapture(e.pointerId);
      const move = (ev) => { view.x = ev.clientX - start.x; view.y = ev.clientY - start.y; applyView(); };
      const up = () => {
        scroller.classList.remove('panning');
        scroller.removeEventListener('pointermove', move);
        scroller.removeEventListener('pointerup', up);
        scroller.removeEventListener('pointercancel', up);
      };
      scroller.addEventListener('pointermove', move);
      scroller.addEventListener('pointerup', up);
      scroller.addEventListener('pointercancel', up);
    }, true);
    const typing = (e) => e.target.closest && e.target.closest('input, select, textarea, button');
    function onKey(e) {
      if (e.code === 'Space' && !typing(e)) {
        e.preventDefault();
        spaceDown = e.type === 'keydown';
        scroller.classList.toggle('space', spaceDown);
        return;
      }
      if (e.type !== 'keydown' || !(e.ctrlKey || e.metaKey)) return;
      if (e.key === '=' || e.key === '+') { e.preventDefault(); zoomTo(view.z * 1.2); }
      else if (e.key === '-') { e.preventDefault(); zoomTo(view.z / 1.2); }
      else if (e.key === '0') { e.preventDefault(); zoomTo(1); }
    }

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
      const add = (name, bundle) => { if (name && !apps.some((a) => a.name.toLowerCase() === name.toLowerCase())) apps.push({ name, bundle: bundle || '', down: 0, up: 0, interfaces: [], idle: true }); };
      for (const k of n.known_apps || []) add(k.name, k.bundle);
      for (const a of n.apps || []) add(a.name);
      for (const w of n.wires || []) add(w.app);
      for (const a of apps) {
        const now = (n.apps || []).find((x) => x.name.toLowerCase() === a.name.toLowerCase());
        if (now) Object.assign(a, now, { bundle: now.bundle || a.bundle, idle: !(now.down || now.up) && !(now.interfaces || []).length });
        a.wire = (n.wires || []).find((w) => w.app.toLowerCase() === a.name.toLowerCase());
      }
      return { links, merge: merge.length >= 2 ? merge : [], apps };
    };

    // Places: the circles down the left, the apps in columns to their right, each until it is moved.
    const layout = (m) => {
      const at = {};
      let y = 40;
      if (m.merge.length) {
        const kept = placed('merge:size');
        const size = Math.min(MERGE_MAX, Math.max(MERGE_MIN, kept ? kept.x : m.merge.length * (C + 16) + 90));
        at.merge = Object.assign({}, placed('merge') || { x: 40, y }, { size });
        y += size + 40;
        m.merge.forEach((name, i) => {
          // Where it was put inside the circle, else in a row across its middle.
          const rel = inCircle(placed('in:' + name) || { x: size / 2 + (i - (m.merge.length - 1) / 2) * (C + 16) - C / 2, y: size / 2 - C / 2 + 6 }, size);
          at['link:' + name] = { x: at.merge.x + rel.x, y: at.merge.y + rel.y, rel, inside: true, d: C };
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
        // The ring of a circle or the edge of a box starts a wire, its middle moves it.
        el.addEventListener('pointerdown', (e) => {
          if (e.target.closest('.network-port, button, .network-grip')) return;
          if (onEdge(e, el)) startWire(e, id); else startMove(e, id);
        });
        el.addEventListener('pointermove', (e) => { if (!moving && !wiring) el.classList.toggle('edge', onEdge(e, el)); });
        el.addEventListener('pointerleave', () => el.classList.remove('edge'));
        if (kind === 'app') {
          el.tabIndex = 0;
          // Enter only: Space is held to move the view.
          el.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); pick(id.slice(4)); } });
        }
        layer.append(el);
        els.set(id, el);
      }
      el.className = 'network-node ' + kind + ' ' + cls;
      return el;
    };
    // Within this many pixels of the screen of a node's rim counts as its rim, whatever the zoom.
    const RIM = 14;
    function onEdge(e, el) {
      const r = el.getBoundingClientRect();
      if (el.dataset.kind === 'app') {
        const d = Math.min(e.clientX - r.left, r.right - e.clientX, e.clientY - r.top, r.bottom - e.clientY);
        return d <= Math.min(RIM, r.height / 3);
      }
      const R = r.width / 2;
      const d = Math.hypot(e.clientX - (r.left + R), e.clientY - (r.top + R));
      // The Combined circle is large, so only a band along its line; a connection's whole outer ring.
      return d <= R + 2 && R - d <= (el.dataset.kind === 'merge' ? RIM + 4 : Math.max(RIM, R * 0.32));
    }
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
      // The header stays one short line: the how-to is the title's tooltip, and only a paused state is said in it.
      title.title = control ? t('network.drag') : t('network.watching') + ' ' + t('network.drag');
      note.textContent = n.paused ? t('network.paused') : '';
      note.title = note.textContent;
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
        if (!el.querySelector('.network-grip')) {
          const grip = h('span', { class: 'network-grip', title: t('network.merge.resize') });
          grip.addEventListener('pointerdown', (e) => startResize(e));
          el.append(grip);
        }
        put(el, at.merge, at.merge.size);
        seen.add('merge');
      }
      for (const l of m.links) {
        const id = 'link:' + l.name;
        const q = l.away ? 'unknown' : l.quality || 'unknown';
        const el = node(id, 'link', (l.primary ? 'primary ' : '') + (l.away ? 'away ' : '') + (at[id].inside ? 'inside ' : '') + 'q-' + q);
        const said = q === 'unknown' ? '' : t('network.q.' + q, { ms: l.rtt, loss: Math.round((l.loss || 0) * 100) });
        el.title = [l.primary ? t('network.link.primary') : l.away ? t('network.link.away') : '', said].filter(Boolean).join('\n');
        el.firstChild.replaceChildren(
          icon(linkSymbol(l), 'network-link-glyph'),
          h('div', { class: 'network-node-title', text: l.label || l.name }),
          h('div', { class: 'network-node-sub', text: l.away ? t('network.link.away') : l.name }),
          h('div', { class: 'network-node-speed', text: l.away ? '' : speed(l.down) + (l.rtt && !at[id].inside ? ', ' + l.rtt + ' ms' : '') }));
        put(el, at[id], at[id].d);
        seen.add(id);
      }
      for (const a of m.apps) {
        const id = 'app:' + a.name;
        const el = node(id, 'app', (a.wire ? 'wired ' : '') + (a.idle ? 'idle ' : '') + (picked === a.name ? 'picked' : ''));
        el.title = a.idle ? t('network.app.idle') : '';
        el.firstChild.replaceChildren(
          // The glyph lies under the icon, so an app whose icon cannot be had still shows one.
          h('span', { class: 'network-app-icon' }, icon('app.dashed', 'network-app-glyph'), a.bundle ? iconImg(a.bundle) : null),
          h('div', { class: 'network-app-text' },
            h('div', { class: 'network-node-title', text: a.name }),
            h('div', { class: 'network-node-sub', text: speed(a.down) + ' / ' + speed(a.up) + (a.wire ? '  ' + (a.wire.target === 'block' ? t('network.blocked') : t('network.priority.' + a.wire.priority)) : '') + (a.refused ? '  ' + t('network.refused', { n: a.refused }) : '') })));
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
      const d = p.d || D;
      return { x: p.x + d, y: p.y + d / 2 };
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
    // What moves on each wire: its path, how fast down (connection to app) and up (app to connection), its colour.
    let flows = [];
    const reduced = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    // The quality of where a wire comes from: a connection's own, the best of those in the Combined circle.
    const RANK = { good: 3, fair: 2, poor: 1, down: 0 };
    const qOf = (id) => {
      const links = current.m.links;
      const of = (name) => { const l = links.find((x) => x.name === name); return l && !l.away && l.quality in RANK ? l.quality : null; };
      if (id === 'merge') return current.m.merge.map(of).filter(Boolean).sort((a, b) => RANK[b] - RANK[a])[0] || 'unknown';
      return of(id.slice(5)) || 'unknown';
    };
    const flowLine = (fromId, to, cls, width, a) => {
      const q = qOf(fromId);
      const p = line(out(fromId), to, cls + ' q-' + q, width, true);
      arrow(p, cls);
      // Nothing moves on a wire whose connection gets no answer.
      if ((a.down || 0) + (a.up || 0) > 500 && q !== 'down') flows.push({ path: p, down: a.down || 0, up: a.up || 0, width, cls, q, dots: [] });
      return p;
    };
    // A small head at the app's end, so the way a wire goes reads even when nothing moves on it.
    function arrow(p, cls) {
      const len = p.getTotalLength ? p.getTotalLength() : 0;
      if (len < 24) return;
      const tip = p.getPointAtLength(len), back = p.getPointAtLength(len - 9);
      const ang = Math.atan2(tip.y - back.y, tip.x - back.x), s = 6;
      const pt = (r, k) => (tip.x + Math.cos(ang + r) * k) + ',' + (tip.y + Math.sin(ang + r) * k);
      const head = document.createElementNS(NS, 'polygon');
      head.setAttribute('points', [pt(0, 0), pt(Math.PI - 0.5, s * 1.6), pt(Math.PI + 0.5, s * 1.6)].join(' '));
      head.setAttribute('class', 'network-arrow ' + cls.replace('network-wire', '').trim());
      svg.append(head);
    }
    function wires() {
      svg.replaceChildren();
      flows = [];
      const { m, at } = current;
      const control = N().control && N().running;
      // The straight wire between the connections of the Combined circle.
      for (let i = 1; i < m.merge.length; i++) {
        // From edge to edge, along the line between the two middles, wherever they were put.
        const a = at['link:' + m.merge[i - 1]], b = at['link:' + m.merge[i]];
        const ca = { x: a.x + C / 2, y: a.y + C / 2 }, cb = { x: b.x + C / 2, y: b.y + C / 2 };
        const d = Math.hypot(cb.x - ca.x, cb.y - ca.y) || 1;
        const ux = (cb.x - ca.x) / d, uy = (cb.y - ca.y) / d;
        if (d > C) line({ x: ca.x + ux * C / 2, y: ca.y + uy * C / 2 }, { x: cb.x - ux * C / 2, y: cb.y - uy * C / 2 }, 'network-wire join', 3);
      }
      for (const a of m.apps) {
        const to = into('app:' + a.name);
        if (!to) continue;
        if (a.wire && a.wire.target !== 'system') {
          const fromId = a.wire.target === 'merge' ? (m.merge.length ? 'merge' : null) : a.wire.target;
          if (fromId && out(fromId)) flowLine(fromId, to, 'network-wire ' + (control ? a.wire.priority : 'planned'), control ? thick(a.down) : 1.8, a);
          continue;
        }
        if (a.idle) continue;
        // Wired by itself: the connections its sockets were seen on, else the one macOS uses.
        const seen = (a.interfaces || []).map((x) => 'link:' + x.name).filter((id) => at[id]);
        const primary = m.links.find((l) => l.primary);
        const froms = seen.length ? seen : primary ? ['link:' + primary.name] : [];
        for (const id of froms) flowLine(id, to, 'network-wire seen', thick(a.down), a);
      }
      // The dots: made here, moved by the clock in tick(), so a redraw every second does not make them jump.
      for (const f of flows) {
        f.len = f.path.getTotalLength();
        const make = (dir, rate) => {
          if (rate <= 500) return;
          const gap = dir > 0 ? 34 : 46;
          const n = Math.max(1, Math.floor(f.len / gap));
          // Faster with more traffic: from a slow walk to a run, on a log scale.
          const v = 28 + 34 * Math.log10(1 + rate / 2e3);
          for (let i = 0; i < n; i++) {
            const c = document.createElementNS(NS, 'circle');
            c.setAttribute('r', dir > 0 ? Math.max(2.2, f.width * 0.75) : 1.9);
            c.setAttribute('class', 'network-flow ' + (dir > 0 ? 'down ' : 'up ') + f.cls.replace('network-wire', '').trim());
            svg.append(c);
            f.dots.push({ c, dir, gap, v, i });
          }
        };
        make(1, f.down);
        make(-1, f.up);
      }
      tick();
      if (flows.length && !reduced && !ticking) { ticking = true; requestAnimationFrame(loop); }
    }
    let ticking = false;
    function loop() {
      if (!flows.length || !root.isConnected) { ticking = false; return; }
      tick();
      requestAnimationFrame(loop);
    }
    function tick() {
      const now = performance.now() / 1000;
      for (const f of flows) for (const d of f.dots) {
        const span = Math.max(f.len, d.gap);
        // Strong: smooth. Fair: slower. Weak: it stalls and lurches, each dot on a beat of its own, and flickers.
        let time = now;
        if (f.q === 'fair') time = now * 0.7;
        else if (f.q === 'poor') time = now * 0.55 + 0.35 * Math.sin(now * 4.2 + d.i * 1.9) + 0.2 * Math.sin(now * 9.7 + d.i);
        // Evenly spaced along the wire, moving along it; down from the connection's end, up from the app's.
        let s = (((reduced ? 0 : time * d.v) + d.i * d.gap) % span + span) % span;
        if (d.dir < 0) s = span - s;
        const pt = f.path.getPointAtLength(Math.min(s, f.len));
        d.c.setAttribute('cx', pt.x);
        d.c.setAttribute('cy', pt.y);
        // Faded in at the start and out at the end, so dots do not pop.
        const edge = Math.min(s, f.len - s);
        let o = Math.max(0, Math.min(1, edge / 18));
        if (f.q === 'poor' && !reduced) o *= 0.35 + 0.65 * Math.abs(Math.sin(now * 7.3 + d.i * 2.3));
        d.c.style.opacity = o;
      }
    }

    // Moving a node: it follows the pointer, its wires with it, and it lands on the grid.
    function startMove(e, id) {
      if (e.button !== 0) return;
      e.preventDefault();
      const el = els.get(id);
      const inside = current.at[id] && current.at[id].inside;
      const start = { x: e.clientX, y: e.clientY };
      const from = { x: current.at[id].x, y: current.at[id].y };
      let moved = false;
      moving = id;
      el.setPointerCapture(e.pointerId);
      const move = (ev) => {
        const dx = (ev.clientX - start.x) / view.z, dy = (ev.clientY - start.y) / view.z;
        if (!moved && Math.abs(dx) + Math.abs(dy) < 4 / view.z) return;
        moved = true;
        const M = current.at.merge;
        if (inside) {
          // A connection in the Combined circle moves about inside it, never out of it.
          const rel = inCircle({ x: from.x + dx - M.x, y: from.y + dy - M.y }, M.size);
          Object.assign(current.at[id], { rel, x: M.x + rel.x, y: M.y + rel.y });
          put(el, current.at[id], C);
          wires();
          return;
        }
        const p = { x: Math.max(0, from.x + dx), y: Math.max(0, from.y + dy) };
        current.at[id].x = p.x; current.at[id].y = p.y;
        if (id === 'merge') {
          for (const name of current.m.merge) {
            const c = current.at['link:' + name];
            c.x = p.x + c.rel.x; c.y = p.y + c.rel.y;
            put(els.get('link:' + name), c, C);
          }
        }
        put(el, p);
        wires();
      };
      const up = () => {
        el.removeEventListener('pointermove', move);
        el.removeEventListener('pointerup', up);
        el.removeEventListener('pointercancel', up);
        moving = null;
        if (moved && inside) {
          const name = id.slice(5), rel = current.at[id].rel;
          local.set('in:' + name, rel);
          send1('network.place', { id: 'in:' + name, x: rel.x, y: rel.y });
        } else if (moved) {
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

    // The grip of the Combined circle makes it larger or smaller; the connections in it stay inside.
    function startResize(e) {
      if (e.button !== 0) return;
      e.preventDefault();
      e.stopPropagation();
      const grip = e.currentTarget;
      const M = current.at.merge;
      const start = { x: e.clientX, y: e.clientY, size: M.size };
      moving = 'merge';
      grip.setPointerCapture(e.pointerId);
      const names = current.m.merge;
      // Each connection keeps its place relative to the middle of the circle as it grows.
      const offsets = names.map((n) => { const r = current.at['link:' + n].rel; return { x: r.x + C / 2 - M.size / 2, y: r.y + C / 2 - M.size / 2 }; });
      const move = (ev) => {
        const grow = Math.max(ev.clientX - start.x, ev.clientY - start.y) / view.z;
        const size = Math.round(Math.min(MERGE_MAX, Math.max(MERGE_MIN, start.size + grow)));
        M.size = size;
        put(els.get('merge'), M, size);
        names.forEach((n, i) => {
          const rel = inCircle({ x: size / 2 + offsets[i].x - C / 2, y: size / 2 + offsets[i].y - C / 2 }, size);
          Object.assign(current.at['link:' + n], { rel, x: M.x + rel.x, y: M.y + rel.y });
          put(els.get('link:' + n), current.at['link:' + n], C);
        });
        wires();
      };
      const up = () => {
        grip.removeEventListener('pointermove', move);
        grip.removeEventListener('pointerup', up);
        grip.removeEventListener('pointercancel', up);
        moving = null;
        local.set('merge:size', { x: M.size, y: M.size });
        send1('network.place', { id: 'merge:size', x: M.size, y: M.size });
        for (const n of names) {
          const rel = current.at['link:' + n].rel;
          local.set('in:' + n, rel);
          send1('network.place', { id: 'in:' + n, x: rel.x, y: rel.y });
        }
        sync();
      };
      grip.addEventListener('pointermove', move);
      grip.addEventListener('pointerup', up);
      grip.addEventListener('pointercancel', up);
    }

    // Wiring: from the dot of a node to another node.
    function startWire(e, id) {
      if (e.button !== 0) return;
      e.preventDefault();
      e.stopPropagation();
      const box = () => board.getBoundingClientRect();
      const from = id.startsWith('app:') ? into(id) : out(id);
      wiring = { id, line: null };
      // The node under the pointer, else the nearest one within a little of it: a wire need not land on a point.
      const target = (ev) => {
        const over = document.elementFromPoint(ev.clientX, ev.clientY);
        const el = over && over.closest && over.closest('[data-node]');
        if (el && el.dataset.node !== id) return el;
        let best = null, bestD = 24;
        for (const [nid, n] of els) {
          if (nid === id) continue;
          const r = n.getBoundingClientRect();
          const d = Math.hypot(Math.max(r.left - ev.clientX, 0, ev.clientX - r.right), Math.max(r.top - ev.clientY, 0, ev.clientY - r.bottom));
          if (d < bestD) { best = n; bestD = d; }
        }
        return best;
      };
      const move = (ev) => {
        if (wiring.line) wiring.line.remove();
        const b = box();
        wiring.line = line(from, { x: (ev.clientX - b.left) / view.z, y: (ev.clientY - b.top) / view.z }, 'network-wire dragging', 2.5, false);
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

    // Arrange: the connections down the left, the apps in columns to their right, in the order asked for; each box
    // is put there and kept, as if moved by hand.
    function arrange(by) {
      if (!current) return;
      const { m, at } = current;
      const rate = (a) => (a.down || 0) + (a.up || 0);
      const via = (a) => a.wire && a.wire.target !== 'system' ? a.wire.target
        : (a.interfaces || []).length ? 'link:' + a.interfaces[0].name : a.idle ? '~idle' : 'system';
      const places = [];
      let y = 40;
      if (m.merge.length) { places.push(['merge', 40, y]); y += at.merge.size + 40; }
      const order = [];
      if (m.merge.length) order.push('merge');
      for (const l of m.links) {
        if (m.merge.includes(l.name)) { order.push('link:' + l.name); continue; }
        places.push(['link:' + l.name, 60, y]);
        order.push('link:' + l.name);
        y += D + 50;
      }
      const left = 60 + Math.max(D, m.merge.length ? at.merge.size - 20 : 0) + 140;
      const byName = (a, b) => a.name.localeCompare(b.name);
      const byRate = (a, b) => rate(b) - rate(a) || byName(a, b);
      let groups;
      if (by === 'link') {
        const rank = (k) => k === '~idle' ? 1e3 : k === 'system' ? 999 : Math.max(0, order.indexOf(k));
        const map = new Map();
        for (const a of m.apps) { const k = via(a); if (!map.has(k)) map.set(k, []); map.get(k).push(a); }
        groups = [...map.entries()].sort((p, q) => rank(p[0]) - rank(q[0])).map(([, list]) => list.sort(byRate));
      } else {
        groups = [m.apps.slice().sort(by === 'name' ? byName : byRate)];
      }
      const ROWS = 9;
      let col = 0;
      for (const list of groups) {
        list.forEach((a, i) => {
          if (i && i % ROWS === 0) col++;
          places.push(['app:' + a.name, left + col * 220, 40 + (i % ROWS) * 80]);
        });
        col++;
      }
      for (const [id, x, py] of places) {
        const p = { x: snap(x), y: snap(py) };
        local.set(id, p);
        send1('network.place', { id, x: p.x, y: p.y });
      }
      sync();
      fit();
    }

    // ---- the card of one app
    const cardTitle = h('h3');
    const held = h('p', { class: 'network-note' });
    const targetSel = h('select', { class: 'select', 'aria-label': t('network.card.target') });
    const prioSel = h('select', { class: 'select', 'aria-label': t('network.card.priority') });
    for (const p of ['high', 'normal', 'low']) prioSel.append(h('option', { value: p, text: t('network.priority.' + p) }));
    const downIn = h('input', { type: 'number', min: '0', max: '1000000', step: '64', class: 'text-field network-limit', placeholder: t('network.card.limit.none'), 'aria-label': t('network.card.down') });
    const upIn = h('input', { type: 'number', min: '0', max: '1000000', step: '64', class: 'text-field network-limit', placeholder: t('network.card.limit.none'), 'aria-label': t('network.card.up') });
    const quotaIn = h('input', { type: 'number', min: '0', max: '10000000', step: '50', class: 'text-field network-limit', placeholder: t('network.card.limit.none'), 'aria-label': t('network.card.quota') });
    const quotaPeriod = h('select', { class: 'select', 'aria-label': t('network.card.quota.period') },
      ...['day', 'month'].map((v) => h('option', { value: v, text: t('network.period.' + v) })));
    const quotaUsed = h('p', { class: 'network-note' });
    const hostsTitle = h('div', { class: 'network-hosts-title', text: t('network.card.hosts') });
    const hostsBox = h('div', { class: 'network-hosts' });
    const field = (label, control) => h('label', { class: 'network-field' }, h('span', { text: label }), control);
    const limit = (input) => { const v = Math.round(Number(input.value)); return v > 0 ? Math.min(v, 1000000) : 0; };
    const save = button(t('network.card.save'), () => {
      if (!picked) return;
      const quota = Math.round(Number(quotaIn.value));
      send1('network.rule', { app: picked, target: targetSel.value, priority: prioSel.value, down_kbps: limit(downIn), up_kbps: limit(upIn),
        quota_mb: quota > 0 ? Math.min(quota, 10000000) : 0, quota_period: quotaPeriod.value });
    }, { primary: true, small: true });
    const unwire = button(t('network.card.unwire'), () => { if (picked) send1('network.unwire', picked); }, { small: true });
    const shut = button(t('network.close'), () => unpick(), { small: true });
    detail.append(cardTitle, held,
      h('div', { class: 'network-fields' },
        field(t('network.card.target'), targetSel), field(t('network.card.priority'), prioSel),
        field(t('network.card.down'), downIn), field(t('network.card.up'), upIn),
        field(t('network.card.quota'), quotaIn), field(t('network.card.quota.period'), quotaPeriod)),
      quotaUsed, hostsTitle, hostsBox,
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
      const blocked = wire && wire.target === 'block';
      if (blocked) held.textContent = t('network.card.block.note');
      held.hidden = !held.textContent;
      const q = ((n.policy || {}).quotas || []).find((x) => x.app.toLowerCase() === app.toLowerCase());
      quotaUsed.textContent = q ? t('network.card.quota.used', { used: size(q.used), quota: size(q.quota) }) : '';
      quotaUsed.hidden = !quotaUsed.textContent;
      const hosts = (a && a.hosts) || [];
      hostsBox.replaceChildren(...(hosts.length
        ? hosts.slice(0, 8).map((x) => h('div', { class: 'network-host' }, h('span', { class: 'network-host-name', text: x.name }), h('span', { class: 'network-host-size', text: size((x.received || 0) + (x.sent || 0)) })))
        : [h('div', { class: 'row-desc', text: t('network.card.hosts.none') })]));
      if (!fresh) return;
      const links = current ? current.m.links : [];
      const options = [['system', t('network.hub.system')], ['block', t('network.hub.block')], ['merge', (n.merge || []).length >= 2 ? t('network.hub.merge') + ' (' + n.merge.map(linkName).join(' + ') + ')' : t('network.hub.merge.all')],
        ...links.map((l) => ['link:' + l.name, (l.label || l.name) + ' (' + l.name + ')'])];
      targetSel.replaceChildren(...options.map(([v, label]) => h('option', { value: v, text: label })));
      targetSel.value = wire ? wire.target : 'system';
      prioSel.value = wire ? wire.priority : 'normal';
      downIn.value = wire && wire.down_kbps ? wire.down_kbps : '';
      upIn.value = wire && wire.up_kbps ? wire.up_kbps : '';
      quotaIn.value = wire && wire.quota_mb ? wire.quota_mb : '';
      quotaPeriod.value = wire ? wire.quota_period : 'month';
      unwire.hidden = !wire;
    }

    // Only the canvas window listens to the whole window; the self-test draws the canvas inside the settings page.
    if (window.__canvas) {
      window.addEventListener('keydown', onKey);
      window.addEventListener('keyup', onKey);
      window.addEventListener('blur', () => { spaceDown = false; scroller.classList.remove('space'); });
      window.addEventListener('keydown', (e) => {
        if (e.key !== 'Escape') return;
        if (!detail.hidden) unpick(); else send({ t: 'action', name: 'network_canvas_close' });
      });
      window.addEventListener('resize', () => sync());
    }
    applyView();
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
    expect(canvas.querySelector('.network-node.app img[data-id="/Applications/Spotify.app"]'), 'network: an app shows its icon, an idle one too');
    expect(canvas.querySelectorAll('.network-node.link .glyph').length === 2, 'network: each connection shows its glyph');
    const steam = nodes('app').find((n) => n.dataset.node === 'app:Steam');
    expect(steam && steam.style.left === '560px' && steam.style.top === '80px', 'network: a box stays where it was put');
    expect(canvas.querySelectorAll('.network-wire.join').length === 1, 'network: the straight wire joins the two connections');
    const merge = canvas.querySelector('.network-node.merge');
    const inner = nodes('link').filter((n) => n.classList.contains('inside'));
    expect(inner.every((n) => parseFloat(n.style.width) < 96), 'network: the connections in the circle are smaller than those on their own');
    const inRect = (n) => { const a = n.getBoundingClientRect(), b = merge.getBoundingClientRect(); return a.left >= b.left && a.right <= b.right && a.top >= b.top && a.bottom <= b.bottom; };
    expect(inner.every(inRect), 'network: they sit inside the circle');
    expect(merge.querySelector('.network-grip'), 'network: the circle has a grip to resize it');
    expect(canvas.querySelectorAll('.network-wire').length >= 4, 'network: the apps are wired');
    expect(canvas.querySelectorAll('.network-flow.down').length > 0 && canvas.querySelectorAll('.network-flow.up').length > 0, 'network: dots go down and up the wires that carry traffic');
    expect(canvas.querySelectorAll('.network-arrow').length >= 4, 'network: each wire shows which way it goes');
    const scroll = canvas.querySelector('.network-canvas-scroll');
    const sb = scroll.getBoundingClientRect();
    scroll.dispatchEvent(new WheelEvent('wheel', { deltaY: -50, ctrlKey: true, clientX: sb.left + 100, clientY: sb.top + 100, bubbles: true, cancelable: true }));
    expect(/scale\((1\.[1-9]|[2-9])/.test(canvas.querySelector('.network-canvas-board').style.transform), 'network: Control and the wheel zoom in');
    sent.length = 0;
    const arrangeSel = canvas.querySelector('.network-arrange');
    arrangeSel.value = 'name';
    arrangeSel.dispatchEvent(new Event('change'));
    expect(sent.some((m) => m.path === 'network.place' && m.value.id === 'app:Steam') && arrangeSel.value === '', 'network: Arrange puts every box and the menu goes back to its title');
    sent.length = 0;
    canvas.querySelector('.network-node.merge .btn').click();
    expect(sent.some((m) => m.path === 'network.merge' && Array.isArray(m.value) && m.value.length === 0), 'network: the circle can be taken apart');
    sent.length = 0;
    [...canvas.querySelectorAll('.btn')].find((b) => b.textContent === t('network.close')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'network_canvas_close'), 'network: the window has its close button');
    holder.remove();
    syncs.splice(syncsBefore);

    const phoneCard = page.querySelector('.phone-card');
    const btn = (text) => [...phoneCard.querySelectorAll('.btn')].find((b) => b.textContent === text && b.style.display !== 'none');
    const shown = () => [...phoneCard.querySelectorAll('.btn')].filter((b) => b.style.display !== 'none').length;
    expect(shown() === 3, 'network: a ready phone offers to start, to turn its VPN off and to test, nothing else: ' + shown());
    expect(phoneCard.dataset.state === 'ready' && phoneCard.textContent.includes('SM G991B') && btn(t('network.tether.start')), 'network: a phone plugged in is ready to share, by its model');
    expect(!phoneCard.textContent.toLowerCase().includes('gnirehtet'), 'network: Phone sharing does not show the tool it runs');
    expect(phoneCard.querySelector('.phone-test').textContent.includes('23.4 ms'), 'network: the last test of the connection is shown with its time');
    sent.length = 0;
    btn(t('network.tether.test')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'network_tether_test'), 'network: the connection can be tested');
    S.network.tether.running = true; runSyncs();
    expect(phoneCard.dataset.state === 'running' && !btn(t('network.tether.start')) && btn(t('network.tether.stop')), 'network: sharing shows as connected, with its stop button');
    expect(!phoneCard.querySelector('.phone-meter').hidden && phoneCard.querySelector('.phone-meter').textContent.includes('2.4 MB/s'), 'network: the meter shows what the phone moves while it shares');
    S.network.tether.test = { state: 'running' }; runSyncs();
    expect(!btn(t('network.tether.test')) && phoneCard.textContent.includes(t('network.tether.testing')), 'network: a test that runs says so and cannot start twice');
    S.network.tether.test = { state: 'done', phone: 'SM G991B', verdict: 'none', loss: 100, ms: null }; runSyncs();
    expect(phoneCard.querySelector('.phone-test').dataset.verdict === 'none', 'network: a phone that does not reach the internet is said so');
    sent.length = 0;
    btn(t('network.tether.stop')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'network_tether_stop'), 'network: stop sends its action');
    S.network.tether.phones = []; runSyncs();
    expect(phoneCard.dataset.state === 'lost', 'network: a phone unplugged while sharing shows as lost');
    S.network.tether.running = false; S.network.tether.unauthorized = 1; runSyncs();
    expect(phoneCard.dataset.state === 'unauthorized', 'network: a phone that has not allowed this Mac says so');
    S.network.tether.unauthorized = 0; S.network.tether.adb = false; runSyncs();
    expect(phoneCard.dataset.state === 'missing' && !phoneCard.querySelector('.phone-install').hidden, 'network: the missing tools come with the command that installs them');
    S.network.tether.adb = true; S.network.tether.phones = [{ serial: 'R58M12ABC', model: 'SM G991B' }]; runSyncs();
    S.network.status = 'denied'; runSyncs();
    expect(page.textContent.includes(t('network.status.denied')), 'network: a refused password says what to do');
    S.network.status = 'on'; runSyncs();
  },
});

// The glyph of a connection, from what System Settings calls it.
function linkSymbol(l) {
  const label = String(l.label || '').toLowerCase();
  if (label.includes('wi-fi') || label.includes('wifi') || label.includes('airport')) return 'wifi';
  if (label.includes('iphone') || label.includes('android') || label.includes('phone')) return 'iphone';
  if (/ethernet|lan|usb|thunderbolt|dock|adapter/.test(label)) return 'cable.connector';
  return 'network';
}

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
