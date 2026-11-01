# Kế hoạch: mở rộng trang Mạng và các công cụ mới

Kế hoạch cho các gợi ý trong đợt research ngày 01/11/2026, bỏ hai mục người dùng không chọn: bộ luật theo từng mạng (A7) và quét thiết bị trong mạng LAN (A8). Chưa viết dòng mã nào.

Nguồn so sánh: trang giới thiệu của Little Snitch, TripMode, Bytetally, Throttlr, NetCurb, WiFi Explorer, Notchy (whats-new, blog 20/7/2026, changelog 2.0.2), NotchNook (1.6.3 đến 1.6.16), RewriteBar, và hai bài Show HN "Baa" và "Muster". Mô tả tính năng là lời của chính hãng, chưa dùng thử.

## Nguyên tắc chung

Giống `docs/feature-gap.md`:

1. Viết lại độc lập, không lấy mã của app khác.
2. Mỗi tính năng tắt theo mặc định, đủ hai ngôn ngữ (`i18n.rs`, từ điển trong `web/app.js`), có kiểm thử. Logic thuần nằm trong crate `core-*` hoặc `logic.rs` của feature, phần macOS nằm trong `macos-sys`, trang Cài đặt nằm trong `web/features/<tên>.js` cùng `.fixture.json`.
3. Ghi vào `docs/configuration.md`, `docs/verification.md` và `docs/releases/<phiên bản>.md`.
4. Không dùng API riêng tư, không đổi cách phát hành (vẫn ký ad hoc, không Network Extension). Mọi việc chặn hay giới hạn chỉ có tác dụng với app đi qua proxy hệ thống, như trang Mạng hiện nay.
5. Mỗi mục có một ví dụ `render_*` hoặc một script `check-*.sh` như các tab khác, để kiểm tra rằng tính năng không làm gì khi tắt.

## Thứ tự các đợt

| Đợt | Mục | Ước lượng |
|---|---|---|
| 1. Nền của trang Mạng | A1 Lịch sử, A2 Đo tốc độ, A6 Thẻ Wi-Fi, A11 Xuất dữ liệu | 4 đến 5 ngày |
| 2. Kiểm soát data | A3 Chặn, hạn mức, chế độ tiết kiệm data; A10 Cảnh báo tải lên bất thường | 4 đến 5 ngày |
| 3. Mạng, phần còn lại | A4 Tên miền theo app, A5 Đảo mất mạng và Wi-Fi cần đăng nhập, A9 DNS mã hóa | 3 đến 4 ngày |
| 4. Việc nhỏ cho cả app | B7 Số chưa đọc, B5 Tốc độ video, B2 Nhảy tới agent đang chờ, B6 Chặn camera | 3 đến 4 ngày |
| 5. Việc vừa | B1 Dọn tiến trình agent, B4 Viết lại bằng AI, B8 Thêm việc nhanh | 5 đến 6 ngày |
| 6. Việc lớn | B3 Bảng lệnh và `figghost://`, B10 Gương camera và máy nhắc chữ, B9 Terminal trong notch | 8 đến 12 ngày |

A1 phải đi trước vì A10, A11 và phần báo cáo của A3 đều đọc dữ liệu của nó. Các đợt 4 đến 6 không phụ thuộc đợt 1 đến 3, nên làm song song được nếu chia người. Các con số ước lượng chưa tính thời gian chờ người dùng thử.

## Đợt 1. Nền của trang Mạng

### A1. Lịch sử dùng mạng

Mục tiêu: hôm nay, 7 ngày, 30 ngày; theo app và theo đường; app dùng nhiều nhất. Hiện tổng chỉ nằm trong bộ nhớ và mất khi đóng trang.

Cách làm:
- `core-netcontrol/src/history.rs` (mới, thuần): `History { days: BTreeMap<Date, HashMap<App, HashMap<Interface, (in, out)>>> }`, cộng phần chênh mỗi lần đọc, cắt ngày theo nửa đêm giờ máy, giữ 90 ngày.
- Tách việc đọc `nettop` của `monitor.rs` khỏi việc trang đang mở: khi `network.history` bật, đọc nền mỗi `network.history_seconds` (5 mặc định; 2, 5 hoặc 10). `nettop -L 1` tốn khoảng 30 ms mỗi lần trên máy này.
- Ghi `network-history.json` trong thư mục dữ liệu, mỗi phút một lần và khi app thoát, ghi cả file hoặc không ghi gì (cách của `screentime.json`). File hỏng ở đâu thì giữ phần còn đúng.
- Trang Mạng có thêm khung "Tổng kết": ba khoảng thời gian, danh sách app có thanh tỷ lệ, chia theo đường, nút Xóa hết.

Kiểm thử: cộng phần chênh khi một socket biến mất (không được trừ), qua nửa đêm, cắt ngày cũ, đọc file hỏng. Thêm `scripts/check-network-history.sh`: tắt thì không chạy `nettop`, bật thì có file sau một phút.

Rủi ro: đọc 5 giây một lần thì bỏ sót socket mở rồi đóng giữa hai lần đọc. Phải đo chênh lệch với cách đọc mỗi giây trước khi chọn mặc định.

### A2. Đo tốc độ từng đường

Mục tiêu: nút "Đo" trên mỗi nút đường của sơ đồ, cho tốc độ tải xuống, tải lên và độ trễ khi đang tải.

Cách làm:
- Chạy `networkQuality -c -I <interface>`, đường dẫn cố định, không qua shell, giới hạn 60 giây, hủy khi đóng trang.
- `core-netcontrol/src/speedtest.rs`: đọc JSON (`dl_throughput`, `ul_throughput`, `responsiveness`, `base_rtt`), có kiểm thử với một kết quả thật lưu làm fixture.
- Hiện kết quả trên nút đường, cạnh vòng chất lượng đã có. Chỉ giữ trong bộ nhớ.
- Trên đường điện thoại (iPhone USB, Android, hotspot) phải hỏi lại trước khi đo, vì một lần đo tải hàng trăm MB.

Cần thử: `-I` có chạy với đường iPhone USB không (chưa thử).

### A6. Thẻ Wi-Fi

Mục tiêu: cường độ sóng (RSSI), nhiễu, SNR, kênh, băng tần, độ rộng kênh, tốc độ liên kết, chuẩn Wi-Fi.

Cách làm:
- `macos-sys/src/wifi.rs` qua CoreWLAN (`CWWiFiClient.sharedWiFiClient().interface()`): `rssiValue`, `noiseMeasurement`, `wlanChannel`, `transmitRate`, `activePHYMode`. Các giá trị này không cần quyền.
- Không đọc SSID, vì từ macOS 14 việc đó cần quyền Vị trí. Nút vẫn ghi "Wi-Fi" như hiện nay.
- Hàm thuần xếp loại SNR thành tốt, khá hoặc yếu, có kiểm thử.
- Chỉ đọc khi trang mở, 2 giây một lần. Hiện trong bảng chi tiết khi bấm vào nút Wi-Fi.

### A11. Xuất dữ liệu

Mục tiêu: nút Xuất trong khung Tổng kết, chọn CSV (`ngày, app, interface, bytes_in, bytes_out`) hoặc JSON, theo ngày hoặc theo tháng.

Cách làm: hàm thuần từ `History` sang chuỗi CSV hoặc JSON, có kiểm thử cho dấu phẩy trong tên app. Lưu bằng hộp thoại lưu tệp sẵn có.

Phần "điều khiển qua Shortcuts" chuyển sang B3, vì cần `figghost://`.

## Đợt 2. Kiểm soát data

### A3. Chặn, hạn mức và chế độ tiết kiệm data

Mục tiêu: khi Mac dùng mạng điện thoại thì chỉ những app được phép mới lên mạng, có hạn mức GB và báo trước khi hết.

Cách làm:
1. Đích mới `block` trong `rules.rs` (`Target::Block`, id `block`). Proxy từ chối kết nối: SOCKS5 trả mã 0x02, HTTP CONNECT trả 403. Trên sơ đồ có thêm một nút "Chặn" để kéo dây vào.
2. Hạn mức cho từng dây: `quota_mb` và `period` (`day` hoặc `month`). Đếm bằng bộ đếm của proxy. Khi hết thì chuyển dây sang chặn đến kỳ sau, và hiện đảo trên notch.
3. Chế độ tiết kiệm data: `network.metered` gồm `links` (interface tính là mạng điện thoại), `auto` (tự nhận diện), `allow` (app được phép), `cap_mb` và `period` cho cả đường.
   - Tự nhận diện: nhãn "iPhone USB" và giao diện chia mạng của Android đã có. Hotspot của iPhone qua Wi-Fi được nhận bằng cổng mặc định 172.20.10.1. Đây chỉ là phép đoán, nên người dùng luôn bật tắt được bằng tay trên từng nút đường.
   - Khi đường chính là mạng điện thoại, app không nằm trong `allow` bị chặn. Báo trên notch ở 80% và 100% hạn mức của đường.
   - Đếm theo đường thì dùng `nettop`, nên cả app không đi qua proxy cũng được tính. Khi hết hạn mức, những app đó chỉ được cảnh báo, không bị chặn; trang phải ghi rõ điều này.
4. Cần bật `control`. Khi `control` tắt, trang chỉ đếm và cảnh báo.

Kiểm thử trong `core-netcontrol`: đổi kỳ hạn mức (nửa đêm, đầu tháng), quyết định chặn, nhận diện đường điện thoại từ danh sách đường giả. Kiểm thử proxy: một kết nối tới app bị chặn nhận đúng mã từ chối.

### A10. Cảnh báo tải lên bất thường

Mục tiêu: báo khi một app tải lên nhiều hơn hẳn thường lệ của chính nó.

Cách làm: hàm thuần trên dữ liệu của A1, lấy trung vị lượng tải lên mỗi giờ của app trong 7 ngày. Nếu 5 phút gần nhất vượt `max(50 MB, 10 lần mức thường)` thì hiện đảo trên notch, có nút "Bỏ qua app này". App chưa đủ một ngày dữ liệu thì không xét. Có kiểm thử cho app mới, app tải lên đều đặn (sao lưu) và một đợt tăng đột ngột.

## Đợt 3. Mạng, phần còn lại

### A4. Tên miền theo app

Mục tiêu: bấm vào một app trên sơ đồ để thấy các tên miền nó kết nối tới, kèm lượng dữ liệu và số kết nối.

Cách làm:
- App đi qua proxy: tên miền có sẵn trong yêu cầu SOCKS5 (kiểu địa chỉ là tên miền) hoặc CONNECT. `handshake.rs` đã đọc ra `Host`, chỉ cần ghi lại theo app trong `proxy.rs`.
- App không đi qua proxy: chỉ có địa chỉ IP từ `nettop -n`, hiện nguyên địa chỉ, không tra ngược.
- Mặc định chỉ giữ trong bộ nhớ. `network.domains_history` (tắt theo mặc định) thì lưu 20 tên miền đứng đầu mỗi app mỗi ngày vào file của A1. Đây là dữ liệu nhạy cảm, nên nút Xóa hết xóa luôn phần này.

### A5. Đảo khi mất mạng, có mạng lại, và Wi-Fi cần đăng nhập

Mục tiêu: ba đảo mới trong trang Đảo trạng thái: mất Internet, có lại (qua đường nào), Wi-Fi đòi đăng nhập (có nút mở trang đăng nhập).

Cách làm:
- Không hỏi đều đặn. Theo dõi thay đổi đường đi bằng `nw_path_monitor` của Network.framework trong `macos-sys/src/netpath.rs`. Mỗi lần có thay đổi thì thử một kết nối giống phép đo chất lượng hiện có.
- Kiểm tra cổng đăng nhập: tải `http://captive.apple.com/hotspot-detect.html`. Nếu trang trả về không phải "Success" thì là Wi-Fi cần đăng nhập.
- Dùng `islands::show_for`, mỗi đảo có một công tắc: `islands.network_down`, `islands.network_back`, `islands.captive`.
- Không cần bật trang Mạng.

### A9. DNS mã hóa

Mục tiêu: chọn Cloudflare, Quad9, AdGuard (chặn quảng cáo), NextDNS (nhập mã) hoặc một địa chỉ https tùy chọn, rồi bấm Cài.

Cách làm:
- Hàm thuần tạo file `.mobileconfig` loại `com.apple.dnsSettings.managed`, giao thức HTTPS, mã định danh cố định `vn.figghost.dns`. Chỉ nhận địa chỉ `https://`. Có kiểm thử: plist hợp lệ, từ chối địa chỉ http, NextDNS ghép đúng mã.
- Ghi file vào thư mục dữ liệu và mở nó. macOS tự đưa người dùng tới System Settings để duyệt. Từ Big Sur không cài hồ sơ bằng dòng lệnh được, nên đây là cách duy nhất.
- Nút Gỡ chỉ mở đúng trang Device Management trong System Settings.

Rủi ro, phải ghi trên trang:
- DNS mã hóa có thể làm hỏng việc phân giải tên nội bộ khi dùng VPN công ty.
- Chia mạng cho điện thoại lấy máy chủ DNS từ `scutil --dns`. Cần thử lại tính năng đó sau khi cài hồ sơ.

## Đợt 4. Việc nhỏ cho cả app

### B7. Số thông báo chưa đọc cạnh notch

Đếm các thông báo đến sau lần mở tab Thông báo gần nhất, vẽ số ở tai phải của notch đóng. Bấm vào thì mở tab. Chỉ có khi tính năng Thông báo bật (đã có quyền Full Disk Access). Công tắc: `notifications.badge`. Thêm một trạng thái cho `render_closed`.

### B5. Tốc độ phát video trong trình duyệt

- Khi nguồn đang phát là một trình duyệt, tab Media hiện các nút 1x, 1.25x, 1.5x và 2x.
- App gửi tin `{ "type": "speed", "rate": 1.5 }` qua kết nối native messaging đang mở (`connectNative`). Extension đặt `playbackRate` cho thẻ `video` đang phát trong tab đang có âm thanh.
- Extension cần thêm quyền `scripting` và quyền truy cập trang. Khai báo là `optional_host_permissions` và xin lúc dùng lần đầu, để bản cập nhật không bị Chrome tắt.
- Safari không có extension này, nên không hỗ trợ. Không dùng cách chạy JavaScript qua AppleScript, vì cách đó bắt người dùng bật một tùy chọn dành cho nhà phát triển.

### B2. Nhảy tới agent đang chờ

- Thêm trường `terminal` vào sự kiện hook, gồm `ORCA_TERMINAL_HANDLE`, `TERM_PROGRAM` và tty. Lệnh hook trong `core-agents/src/cli.rs` và `pi_extension.ts` đọc các biến này. Thêm trường mới không làm hỏng bản cũ.
- `tracker.rs` giữ trường này cho mỗi phiên.
- Phím tắt `agents.jump_shortcut` (để trống theo mặc định) chọn phiên đang chờ lâu nhất. Bấm vào dòng của phiên trên notch cũng làm như vậy.
  - Nếu có handle của Orca: chạy `orca terminal switch --terminal <handle>` với đối số cố định, rồi đưa Orca lên trước.
  - Nếu không: chỉ đưa app terminal lên trước theo `TERM_PROGRAM`, vì không có cách chung để chọn đúng tab.
- Kiểm thử: `check-agents.py` gửi một sự kiện có handle giả, kiểm tra rằng lệnh `orca` được gọi đúng đối số (dùng một chương trình `orca` giả trong PATH).

### B6. Chế độ chặn camera

Cần thử trước khi cam kết. `camera.rs` chỉ biết camera đang bật, không biết app nào đang dùng, và macOS không có API công khai cho việc đó.

- Bước 1, nửa ngày: tìm cách biết app nào dùng camera mà không cần API riêng tư.
- Nếu có cách: thêm `islands.camera_guard` (`off`, `warn` hoặc `quit`) và danh sách app được phép. App lạ mở camera thì cảnh báo, hoặc thoát nó sau một lần hỏi.
- Nếu không có cách: chỉ làm `warn`, kèm tên app đang ở trước tại thời điểm camera bật, ghi rõ đó là phép đoán. Không bao giờ tự thoát app dựa trên phép đoán.

## Đợt 5. Việc vừa

### B1. Dọn tiến trình agent bỏ lại

Mục tiêu: một mục mới trong trang Mole liệt kê các tiến trình agent bỏ lại, kèm bộ nhớ, tuổi, cổng đang mở, và nút dừng.

Cách làm:
- `macos-sys/src/process.rs` có sẵn libproc. Thêm hàm liệt kê tiến trình của người dùng hiện tại với pid, ppid, thời điểm bắt đầu, RSS, đường dẫn, đối số (`KERN_PROCARGS2`) và cổng đang nghe.
- Bộ phân loại thuần trong `features/mole/agents.rs`, có kiểm thử. Một tiến trình được xem là bị bỏ lại khi:
  - cha của nó là launchd (ppid 1);
  - đối số khớp các mẫu đã biết: máy chủ MCP (`*-mcp`, `mcp-server-*`, `@modelcontextprotocol/*`), dev server (`vite`, `next dev`, `webpack serve`), Chromium không giao diện của Playwright hoặc Puppeteer, `node` hoặc `bun` chạy trong `~/.npm/_npx`;
  - đã chạy hơn 10 phút.
  - Loại trừ mọi thứ trong `/System`, `/Library` và bên trong một app bundle, cùng tiến trình của các phiên agent mà tab AI agents đang thấy còn sống.
- Không bao giờ tự dừng. Người dùng chọn rồi bấm Dừng: gửi SIGTERM, 5 giây sau mới gửi SIGKILL.
- Tab AI agents có thêm một dòng "N tiến trình bị bỏ lại, X GB" dẫn tới trang Mole.

### B4. Viết lại đoạn đang chọn bằng AI

- Phím tắt `rewrite.shortcut` đọc đoạn đang chọn bằng `selection.rs` (như Translate) và mở cửa sổ kính cạnh con trỏ (dựa trên `translate_ui.rs`).
- Bốn lệnh mặc định: Sửa lỗi, Trang trọng hơn, Ngắn gọn hơn, Sang tiếng Anh tự nhiên. Có thêm một ô nhập yêu cầu tự do.
- Gọi `core-jarvis::chat::stream` với nhà cung cấp và khóa của Jarvis, kể cả Ollama trên máy. Câu trả lời hiện dần.
- Nút Thay: dán bằng Command+V, lưu và trả lại clipboard, đánh dấu để lịch sử clipboard không giữ (cách của Dictation). Không có quyền Accessibility thì để kết quả trên clipboard.
- Bật tính năng nghĩa là đồng ý gửi đoạn được chọn tới nhà cung cấp, giống cách ghi cho Jarvis.

### B8. Thêm việc nhanh vào Apple Reminders

- Tab Calendar có ô "Thêm việc". Một bộ đọc ngày giờ tiếng Việt và tiếng Anh trong `core-calendar` ("mai 9h", "thứ 6", "tối nay", "tomorrow 5pm") có kiểm thử, và có các nút nhanh: Hôm nay, Tối nay, Mai.
- `eventkit.rs` thêm hàm tạo `EKReminder` trong danh sách mặc định. Quyền truy cập Reminders đã được xin cho Calendar.
- Không làm hẹn giờ dạng bánh xe như NotchNook, vì Pomodoro đã có đếm giờ.

## Đợt 6. Việc lớn

### B3. Bảng lệnh và `figghost://`

- Phím tắt `palette.shortcut` mở một bảng kính (`panel.rs`) có ô tìm.
- Danh sách lệnh: mở một tab của notch, bật tắt từng tính năng, các lệnh của Tiện ích (giữ máy thức, tắt micro, ẩn biểu tượng màn hình nền), chạy Shortcuts đã ghim, và các lệnh mạng (đo tốc độ, chặn một app, bật chế độ tiết kiệm data).
- Bộ khớp thuần, có kiểm thử, khớp được tiếng Việt không dấu: "tat mic" khớp "Tắt micro". Lệnh dùng gần đây đứng trên.
- `figghost://<lệnh>` gọi cùng danh sách lệnh. Nhờ đó Shortcuts, Raycast hay script đều điều khiển được app; phần này hoàn tất mục "điều khiển qua Shortcuts" của A11. Lệnh có tác động (chặn app, bật proxy) phải hỏi xác nhận khi đến từ URL.

### B10. Gương camera và máy nhắc chữ

- Gương: một tab của notch dùng `AVCaptureSession` với lớp xem trước. Camera chỉ mở khi tab hiện và đóng ngay khi tab đóng. Xin quyền bằng `permissions.rs` sẵn có.
- Máy nhắc chữ: một dải chữ ngay dưới camera, đọc từ một ghi chú trong Notes. Có tốc độ cuộn, cỡ chữ, phím tạm dừng; ẩn khỏi ảnh chụp và chia sẻ màn hình (`hide_in_capture`). Cuộn theo giọng nói (`speech.rs`) để bước sau.

### B9. Terminal trong notch

Mục rủi ro nhất, nên làm cuối và làm thử trước.

- Thử (2 ngày): mở PTY bằng `openpty` với shell đăng nhập; dùng crate `vte` làm bộ đọc chuỗi điều khiển cùng một lưới ô tự viết; vẽ lưới bằng Core Text trong một NSView của notch. Đạt khi `vim`, `htop` và màu chạy đúng, gõ tiếng Việt bằng bộ gõ của figghost không vỡ.
- Không đạt thì thay bằng bản nhỏ hơn: chạy một lệnh đã lưu và xem đầu ra cuộn, không nhận phím.
- Không dùng `alacritty_terminal`, vì nó kéo theo nhiều phụ thuộc so với nhu cầu.

## Câu hỏi cần chốt trước khi làm

1. A1: chấp nhận đọc `nettop` nền 5 giây một lần khi bật Lịch sử, hay chỉ ghi khi trang Mạng đang mở?
2. A3: chặn chỉ tác dụng với app đi qua proxy hệ thống. App khác chỉ bị đếm và cảnh báo. Chấp nhận cho bản đầu không?
3. B5: chấp nhận extension xin thêm quyền truy cập trang lúc dùng lần đầu không?
4. B6: nếu không biết chắc app nào dùng camera, chỉ làm cảnh báo có được không?
5. B9: làm thử terminal thật, hay đi thẳng bản chỉ chạy lệnh đã lưu?

## Trạng thái (1/11/2026)

Đã làm theo kế hoạch, trừ những chỗ ghi dưới đây. Kiểm tra: `cargo build --workspace` không cảnh báo, `cargo test --workspace` đạt, bài tự kiểm của trang cài đặt đạt, chi tiết và phần chưa kiểm tra ở [verification.md](verification.md).

Khác với kế hoạch:

- A1: đọc `nettop` nền theo `network.history_seconds` (2, 5 hoặc 10 giây) đúng như phương án đề xuất. Lượng dữ liệu đi qua proxy được tính cho app thật chứ không tính cho Figghost.
- A3: chặn và hạn mức chỉ có tác dụng với app theo proxy hệ thống; app khác chỉ được đếm và cảnh báo. Hạn mức và hạn mức đường mạng buộc phải đếm lịch sử, nên "Xóa lịch sử" cũng đặt lại hạn mức.
- A4: tên miền chỉ giữ trong bộ nhớ (60 mỗi app); chưa làm phần lưu lịch sử tên miền.
- A5: thông báo nằm ở trang Mạng chứ không ở trang Đảo trạng thái, và độc lập với công tắc "Theo dõi mạng". Notch không có nút bấm, nên "mở trang đăng nhập" là một nút ở trang cài đặt.
- A7, A8: bỏ theo yêu cầu.
- B1: nằm trong trang Mole, dùng chung bộ chọn dòng với Docker. Chưa có dòng "N tiến trình bị bỏ lại" trên tab AI agents.
- B2: có dữ liệu terminal của mỗi phiên và hàm đưa terminal lên trước, gọi từ bảng lệnh, địa chỉ `figghost://agents/jump` và action `agents_jump`. Chưa có phím tắt riêng và chưa có nút trên dòng của phiên.
- B3: cửa sổ bảng lệnh là một cửa sổ có thanh tiêu đề chứ không phải panel không viền, vì panel không nhận phím thì không gõ được. Chưa nhớ lệnh dùng gần đây.
- B4: dùng lại cửa sổ kính của Dịch và mô hình mà Jarvis đang dùng.
- B5: tốc độ đặt ở trang Trình duyệt chứ chưa có nút trên tab Media.
- B6: chỉ nêu tên app đang ở trước, như một phỏng đoán; không tự thoát app nào.
- B7, B8: B8 chỉ có ở bảng lệnh và địa chỉ `figghost://remind`, chưa có ô nhập trên tab Lịch.
- B9: làm bản nhỏ hơn đã nêu trong kế hoạch: tab Lệnh chạy lệnh đã lưu, không phải terminal thật.
- B10: gương và máy nhắc chữ là hai cửa sổ nổi chứ không phải tab của notch.
