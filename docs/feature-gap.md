# Bảng đối chiếu tính năng: các app notch so với Figghost

Danh sách tính năng của các app notch và launcher đã khảo sát, đối chiếu với Figghost 1.4.2, kèm mức khả thi và lô thực hiện. Đây là kế hoạch gốc cho yêu cầu "làm đủ tính năng của các app đó".

## Nguyên tắc

1. Viết lại độc lập từ mô tả công khai của từng tính năng. Không lấy mã, hình, âm thanh, tên sản phẩm hay lời quảng cáo. Boring Notch và Atoll là GPL-3.0, còn Figghost là MIT, nên mã của hai app đó không được đưa vào dù chỉ một đoạn.
2. Mỗi tính năng mới tắt theo mặc định, có trang riêng trong Cài đặt, đủ hai ngôn ngữ, có kiểm thử, và được ghi vào `docs/configuration.md` và `docs/verification.md`.
3. Không làm những gì cần API riêng tư, cần quyền Full Disk Access để đọc dữ liệu của app khác, cần chứng nhận mà app ký ad hoc không có, hoặc có rủi ro an toàn (xem mục cuối).

## Nguồn

Danh sách tính năng chính thức của Notchy (notchy.dev, trang chủ liệt kê 74 mục, bản 1.0.178 ngày 25/9/2026), MacNotch (macnotch.io, 19 module), README của Atoll và Boring Notch trên GitHub, trang của DynamicLake, Seam và Alcove, cộng với `docs/research-raycast-powertoys.md` cho Raycast. Đọc ngày 20/10/2026. Mô tả tính năng là lời của chính hãng, chưa dùng thử.

## Cách đọc bảng

- Figghost: Có (đã có đủ), Một phần, Chưa.
- Khả thi: Dễ (một ngày hoặc ít hơn, API công khai), Vừa (vài ngày, cần giao diện mới), Khó (nhiều ngày hoặc phụ thuộc phần cứng, quyền).
- Lô: thứ tự làm. Lô 1 là đợt đầu.

## Phát nhạc và âm thanh

| Tính năng | Có ở | Figghost | Khả thi | Lô |
|---|---|---|---|---|
| Điều khiển Now Playing, ảnh bìa, tua | Tất cả | Có | | |
| Lời bài hát đồng bộ | Notchy, Boring Notch | Có (LRCLIB) | | |
| Sóng nhạc theo âm thanh thật (FFT) | Notchy, Boring Notch, Atoll | Có (thanh trên notch đóng) | | |
| Xem nhanh bài đang phát khi rê chuột lên viên thuốc, có nút điều khiển | Notchy | Chưa | Vừa | 4 |
| Mixer âm lượng từng app, EQ | Notchy | Có (Sound) | | |
| Chuyển thiết bị phát âm thanh bằng một cú chạm | Notchy, Seam | Một phần (đổi đầu ra theo từng app) | Dễ | 3 |
| Pin AirPods từng bên, kết nối một chạm | Notchy, Seam | Một phần (tab Devices đọc pin từng bên tai và hộp sạc theo các khóa của `system_profiler`, chưa thấy trên máy này vì không có AirPods; chưa có nút kết nối một chạm) | Vừa | 3 |
| Pin Magic Mouse, Keyboard, Trackpad, tai nghe Bluetooth | Notchy, Atoll | Có (tab Devices, tắt theo mặc định; mức pin mà thiết bị báo cho hệ thống, hỏi `system_profiler` khi tab mở và nửa phút một lần khi tab hiện; thiết bị không báo thì không có số; không cần quyền) | | Xong |
| Đảo Bluetooth kết nối và ngắt | Boring Notch, Atoll | Có | | |
| Thích bài trong Apple Music, đổi tốc độ video trong trình duyệt | Notchy | Chưa | Khó | - |

## Đảo hệ thống (live activity trên notch)

| Tính năng | Có ở | Figghost | Khả thi | Lô |
|---|---|---|---|---|
| Sạc, đầy pin theo ngưỡng chọn, pin yếu | Notchy, Boring Notch, Atoll, DynamicLake | Có (Đảo trạng thái) | | Xong, lô 1 |
| Caps Lock | Notchy | Có (Đảo trạng thái) | | Xong, lô 1 |
| Camera và micro đang bật (chỉ báo riêng tư) | Notchy, Atoll | Có (Đảo trạng thái) | | Xong, lô 1 |
| Ổ USB cắm vào, an toàn để rút, nút Eject | Notchy | Có (Đảo trạng thái; Eject nằm trong bảng thanh menu, không phải nút trên đảo) | | Xong, lô 1 |
| Tải xong tệp trong Downloads, Reveal in Finder | Notchy, Atoll (beta) | Một phần (có đảo tải xong; chưa có nút Reveal in Finder vì đảo chưa nhận cú bấm riêng) | Vừa | 5 |
| Nhiều hoạt động cùng lúc trên viên thuốc đóng, xếp theo mức quan trọng | Notchy | Một phần (chỉ một chỗ, theo ưu tiên) | Khó | 5 |
| Chuyển đổi Focus (không làm phiền) | Notchy, Atoll, MacNotch | Chưa | Không làm: cần API riêng tư | - |
| Ghi màn hình đang chạy | Atoll | Có (thanh ghi hình) | | |

## Tiện ích hệ thống

| Tính năng | Có ở | Figghost | Khả thi | Lô |
|---|---|---|---|---|
| Giữ máy thức (caffeine) với thời lượng hoặc đến giờ chọn | Notchy | Có (Tiện ích; chọn thời lượng, chưa chọn "đến giờ") | | Xong, lô 1 |
| Tắt tiếng micro toàn hệ thống | Notchy | Có (Tiện ích) | | Xong, lô 1 |
| Khóa bàn phím để lau, đếm ngược, mở bằng Command-Escape | Notchy | Có (Tiện ích) | | Xong, lô 1 |
| HUD phím bấm khi trình chiếu (bỏ qua ô mật khẩu) | Notchy | Một phần (có ở thanh ghi hình) | Vừa | 2 |
| Số liệu hệ thống: CPU từng lõi, bộ nhớ và swap, mạng, ổ đĩa, GPU, nhiệt, sức khỏe pin | Notchy, Atoll, MacNotch, Seam | Có (tab System của notch, tắt theo mặc định, mỗi thẻ bật tắt riêng; nhiệt độ CPU và tốc độ quạt từ 1.6.0, xem dưới) | | Xong |
| Nhiệt độ CPU và tốc độ quạt | Atoll, Stats, Macs Fan Control | Có (thẻ Nhiệt và thẻ Quạt của tab System, chỉ đọc: nhiệt độ die qua cảm biến HID, quạt qua SMC; không cần root, không chỉnh tốc độ quạt) | Ngoại lệ có chủ ý cho nguyên tắc 3, theo yêu cầu | Xong |
| Ẩn khỏi ảnh chụp và chia sẻ màn hình | Notchy | Có (cài đặt Notch, `hide_in_capture`; macOS mới vẫn cho một số app chụp) | | |
| Ẩn notch phần cứng bằng thanh menu đen | Notchy | Chưa | Vừa | 5 |
| Ẩn biểu tượng thanh menu | Notchy | Có | | |
| Tạm tắt notch một lúc (snooze), có phím tắt | MacNotch | Có (Tiện ích; chỉ chặn mở khi rê chuột, bấm và tác tử vẫn mở được) | | Xong |
| Độ sáng màn hình ngoài qua DDC | Notchy, MonitorControl | Có (tab Displays, tắt theo mặc định; độ sáng và độ tương phản qua `IOAVService` trên Apple silicon) | Ngoại lệ có chủ ý cho nguyên tắc 3, theo yêu cầu | Xong |
| Đèn nền bàn phím | Notchy | Chưa | Không làm: cần CoreBrightness riêng tư | - |
| HUD âm lượng, nhiều kiểu | Tất cả | Có (bảy kiểu) | | |
| HUD độ sáng, thay HUD gốc của macOS | Notchy, Boring Notch | Một phần (phím độ sáng chỉnh màn hình rời qua DDC và hiện HUD kiểu bảng âm lượng; màn hình liền máy vẫn dùng HUD gốc) | | Một phần |
| Gương camera | Notchy, Boring Notch, NotchNook | Chưa | Vừa | 4 |
| Nhắc nghỉ mắt toàn màn hình (20-20-20) | MacNotch | Chưa | Vừa | 4 |

## Công việc và năng suất

| Tính năng | Có ở | Figghost | Khả thi | Lô |
|---|---|---|---|---|
| Lịch và nhắc việc hôm nay, nút vào họp | Tất cả | Có | | |
| Thêm nhắc việc nhanh (Apple Reminders) | Notchy, MacNotch | Chưa (chỉ đọc và đánh dấu xong) | Vừa | 4 |
| Bấm giờ Pomodoro: chuỗi phiên, bấm giờ đếm lên, chuỗi ngày, biểu đồ 7 ngày | Notchy, MacNotch, Boring Notch | Có (tab Pomodoro, tắt theo mặc định: chu kỳ làm và nghỉ, vòng nghỉ dài, đồng hồ trong notch đóng, chuỗi ngày, biểu đồ 7 ngày; không có bấm giờ đếm lên) | | Xong |
| Ghi chú nhanh | Notchy, MacNotch | Có (tab Notes, tắt theo mặc định; mỗi ghi chú là một tệp văn bản trong thư mục `notes`) | | Xong |
| Bảng lệnh (phím tắt toàn cục): nhảy tới tab, bật tắt tính năng, chạy lệnh, tính nhanh | Notchy, Raycast | Chưa | Vừa | 2 |
| Mở nhanh app đã ghim trong notch | Notchy, MacNotch | Có (tab Launcher, tắt theo mặc định; app, thư mục và Shortcuts) | | Xong |
| Mở rộng văn bản (snippet) | Notchy, Raycast | Chưa | Vừa | 2 |
| Chạy Apple Shortcuts | Notchy, NotchNook, Alcove | Có (ghim vào tab Launcher, chạy bằng lệnh `shortcuts run`) | | Xong |
| Nhấn đúp một phím bổ trợ để mở notch ("knock" bằng phím) | Notchy | Một phần (đã có bộ nhận nhấn đúp phím ở Copy Stack) | Dễ | 2 |
| Thời tiết | Notchy, MacNotch, Atoll, Seam | Có (tab Weather, tắt theo mặc định; Open-Meteo, không cần khóa; chọn thành phố bằng tìm kiếm, không hỏi vị trí máy) | | Xong |
| Tiến độ trong ngày (lịch, nhắc việc, việc cần làm trên một trục) | MacNotch | Có (tab Day progress, tắt theo mặc định; phần trăm ngày đã trôi qua, trục từ buổi sáng đến giờ đi ngủ với sự kiện và nhắc việc của hôm nay, việc đang diễn ra và việc kế tiếp; đọc đúng các lịch mà tab Calendar đọc) | | Xong |
| Thời gian dùng màn hình theo app và nhóm | MacNotch | Có (tab Screen Time, tắt theo mặc định; theo app và theo nhóm mà app tự khai, hôm nay và 7 ngày; không cần quyền) | | Xong |
| Vòng sức khỏe: nghỉ, nước, thở | MacNotch | Chưa | Vừa | 4 |
| Hàng đợi pull request cần duyệt (GitHub, GitLab, Bitbucket) | MacNotch, Notchy | Có (tab Pull requests, tắt theo mặc định; GitHub qua `gh` và GitLab qua `glab` đã đăng nhập, không giữ khóa; chưa có Bitbucket vì không có công cụ dòng lệnh chung) | | Xong |
| Máy nhắc chữ (teleprompter) dưới camera | Notchy, Moody | Chưa | Vừa | 4 |
| Cổ phiếu | Notchy | Chưa | Vừa, nhu cầu thấp | - |
| Hồ sơ dashboard đổi theo giờ hoặc Focus | MacNotch | Chưa | Vừa | 5 |
| Tìm kiếm và dịch ngay trong notch, chín công cụ tìm | Notchy | Một phần (Smart Select dịch) | Vừa | - |

## Tệp và Shelf

| Tính năng | Có ở | Figghost | Khả thi | Lô |
|---|---|---|---|---|
| Kệ tệp thả và kéo ra, giữ lại sau khi thoát | Tất cả | Có | | |
| Nén, giải nén, đổi định dạng ảnh, thu nhỏ, bỏ nền | Notchy, DynamicLake | Có | | |
| Gửi AirDrop từ shelf | Boring Notch, Notchy | Một phần (đã có ở Drag wheel, chưa có trong menu shelf) | Dễ | 5 |
| Ảnh chụp màn hình tự vào shelf | Notchy | Chưa | Dễ | 5 |
| Lịch sử clipboard, ghim, tìm, hành động thông minh (liên kết, màu, email) | Notchy, DynamicLake | Có (hành động thông minh: một phần) | | |
| Tìm trong chữ nằm trong ảnh đã sao chép (OCR) | Notchy | Chưa | Vừa | 5 |
| Chuyển video thành GIF | Notchy, DynamicLake | Có (Drag wheel, ghi GIF) | | |

## Cử chỉ, hình thức, mở rộng

| Tính năng | Có ở | Figghost | Khả thi | Lô |
|---|---|---|---|---|
| Cử chỉ vuốt trên notch: đổi tab, mở, đóng | Notchy, Atoll, Alcove | Chưa | Vừa | 5 |
| Phản hồi xúc giác | Notchy, NotchNook | Có | | |
| Kiểu kính có tên, kích thước đổi được | Notchy, DynamicLake | Có (glass, clear, black; bốn cỡ) | | |
| Viên thuốc ảo trên Mac không notch và màn ngoài | Notchy, Alcove, DynamicLake | Có (một màn hình) | | |
| Notch trên mọi màn hình cùng lúc | Notchy, MacNotch | Chưa | Khó: đã ghi trong verification.md | 5 |
| Hình động tự chọn cho viên thuốc lúc rảnh | Notchy | Chưa | Khó (cần bộ phát Lottie) | - |
| API cục bộ để script đẩy đảo lên notch | Notchy | Một phần (đã có ổ cắm Unix cho agent) | Vừa | 5 |
| Liên kết `figghost://` | Raycast | Chưa | Dễ | 5 |
| Hệ thống plugin, tiện ích mở rộng | DynamicLake, Boring Notch, Raycast | Chưa | Khó, ngoài phạm vi | - |
| 134 ngôn ngữ | Notchy | Hai ngôn ngữ (Anh, Việt) | Khó: mỗi ngôn ngữ là một bản dịch phải kiểm | - |
| Theo dõi mức dùng AI: Claude Code, Codex | Notchy, Atoll | Có | | |
| Theo dõi mức dùng AI: Cursor, Copilot | Notchy, Atoll | Chưa | Khó: không có nguồn đọc công khai ổn định | - |
| Phê duyệt quyền của agent bằng Allow và Deny | Notchy, MacNotch | Có | | |
| Hook shell: lệnh dài tự thành đảo | Notchy | Chưa | Vừa, can thiệp vào `~/.zshrc` nên cần cân nhắc | - |

## Không làm, và lý do

| Tính năng | Lý do |
|---|---|
| Mở khóa Mac bằng khuôn mặt (Face Unlock của Notchy) | Cần giữ mật khẩu đăng nhập để mở máy; rủi ro an toàn quá lớn so với lợi ích, và Apple Vision không cung cấp cảm biến chiều sâu. |
| Gương chiếu thông báo, đảo tin nhắn có trả lời, đảo cuộc gọi FaceTime bấm nghe hoặc từ chối | Phải đọc cơ sở dữ liệu thông báo bằng Full Disk Access hoặc điều khiển giao diện của app khác qua Accessibility; mong manh và xâm phạm quyền riêng tư. |
| Widget màn hình khóa đầy đủ (đồng hồ, pin, thời tiết, lịch) | Cần cửa sổ ở tầng riêng tư của window server (SkyLight, `CGSSpace`). Figghost đã có lớp màn hình khóa cho widget nhạc từ 1.4.1; mở rộng thêm cùng một cơ chế thì cân nhắc sau. |
| Widget WidgetKit trên màn hình nền | Cần tiện ích mở rộng được ký bằng Developer ID và có chứng nhận; app ký ad hoc không nạp được. |
| Đèn nền bàn phím, độ sáng màn hình liền máy | API riêng tư (CoreBrightness, DisplayServices). Độ sáng màn hình rời và nhiệt độ, quạt đã làm từ 1.6.0 như một ngoại lệ có chủ ý. |
| Chỉnh tốc độ quạt | Cần một tiến trình chạy quyền root và ghi SMC; trên M3 trở lên còn phải vượt khóa của `thermalmonitord`. Rủi ro quá nóng nếu sai. |
| Đọc mức dùng của Cursor và Copilot | Không có nguồn đọc ổn định, công khai. |
| Chặn camera khi có app mở | macOS không có API công khai chặn camera của app khác. Chỉ có thể cảnh báo. |

## Thứ tự thực hiện

1. Lô 1, đã xong: đảo hệ thống (sạc và pin, Caps Lock, camera và micro, ổ USB, tải xong) và tiện ích (giữ máy thức, tắt micro, khóa bàn phím). Dùng chung một chỗ đặt `notch.set_activity` (`features/islands::show_for`), nên mỗi đảo chỉ cần logic phát hiện và một dòng cài đặt. Còn thiếu của lô này: nút Reveal in Finder trên đảo tải xong và nút Eject trên đảo ổ đĩa, cả hai cần đảo nhận một cú bấm riêng (hiện bấm vào đảo chỉ mở notch).
2. Lô 2: đã xong ghi chú nhanh (tab Notes), app ghim và chạy Shortcuts (tab Launcher), tạm tắt notch (Tiện ích). Còn lại: bảng lệnh, snippet, HUD phím bấm, mở notch bằng nhấn đúp phím.
3. Lô 3: đã xong số liệu hệ thống (tab System), thời tiết (tab Weather) và pin thiết bị Bluetooth (tab Devices). Còn lại: nút kết nối một chạm, chuyển thiết bị âm thanh.
4. Lô 4: đã xong thời gian dùng màn hình (tab Screen Time), Pomodoro đầy đủ (tab Pomodoro) và hàng đợi pull request (tab Pull requests). Đã xong thêm tiến độ trong ngày (tab Day progress). Còn lại: nhắc việc nhanh (Todo), vòng sức khỏe và nghỉ mắt, gương camera, teleprompter, xem nhanh bài đang phát.
5. Lô 5: cử chỉ vuốt, AirDrop và ảnh chụp tự vào shelf, tìm chữ trong ảnh clipboard, ẩn khỏi chia sẻ màn hình, ẩn notch phần cứng, API cục bộ và `figghost://`, hồ sơ dashboard, nhiều màn hình.

Mỗi lô gồm: logic thuần trong crate `core-*` có kiểm thử đơn vị, phần macOS trong `macos-sys`, bộ điều khiển trong `app-ui/src/features/<tên>`, trang Cài đặt (`web/features/<tên>.js`, `.css`, `.fixture.json`), chuỗi hai ngôn ngữ, mục trong `docs/configuration.md` và `docs/verification.md`.
