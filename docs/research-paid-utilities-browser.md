# Tiện ích trả phí trên macOS và extension trình duyệt: những gì đưa được vào Figghost

Nghiên cứu các app tiện ích trả phí trên macOS, gồm cả các app đi kèm extension trình duyệt, để chọn những gì Figghost làm được. Tài liệu nối tiếp `docs/feature-gap.md` (các app notch), `docs/research-raycast-powertoys.md` và `docs/research-raycast-2.5.0.md` (Raycast), `docs/research-capture-and-audio.md` (CleanShot X, SoundSource, Boom 3D) và `docs/research-tangerine.md`. App đã khảo sát ở các tài liệu đó không nhắc lại ở đây. Đối chiếu với mã ở bản 2.1.17.

## Nguồn và nguyên tắc

- Trang của nhà phát triển, trang cửa hàng (Chrome Web Store, Firefox Add-ons, App Store, Setapp), các bài so sánh năm 2026, tài liệu Chrome về native messaging và tài liệu MDN về native manifests. Đọc ngày 28/10/2026. Danh sách nguồn ở cuối.
- Giá là giá tham khảo. Nhiều nguồn là blog của chính hãng bán app cạnh tranh, và giữa các nguồn có chỗ lệch; chỗ nào lệch thì ghi cả hai. Ô "chưa kiểm" là chưa tìm được giá từ nguồn đáng tin.
- Mô tả tính năng là lời của chính hãng, chưa dùng thử.
- Giữ nguyên tắc của `feature-gap.md`: viết lại độc lập từ mô tả công khai, không lấy mã, hình hay tên sản phẩm; mỗi tính năng tắt theo mặc định và có trang Cài đặt riêng; không dùng API riêng tư, không cần quyền root, không cần chứng nhận mà app ký ad hoc không có.

## 1. Đã có trong Figghost

Những app trả phí sau đã có tính năng tương ứng, nên không phải khoảng trống.

| App trả phí | Tính năng Figghost |
|---|---|
| Paste, Pastebot | Clipboard (lịch sử, ghim, boards, Copy Stack, MCP) |
| Magnet, BetterSnapTool | Chia cửa sổ |
| iStat Menus, iStatistica | Tab System |
| Lunar Pro, BetterDisplay Pro (phần DDC) | Tab Displays |
| SoundSource, Boom 3D | Sound theo từng app |
| CleanShot X, Shottr, TextSniper | Chụp màn hình, ghi hình, nhận dạng chữ |
| NotchNook, Alcove, MediaMate | Notch, đảo trạng thái, HUD âm lượng |
| Yoink, Dropover, Dropzone | Shelf và Drag wheel |
| TextExpander, Typinator, aText | Gõ tắt |
| Amphetamine (miễn phí), Lungo | Tiện ích: giữ máy thức |
| CleanMyMac | Mole |
| AirBuddy | Tab Devices (chưa có nút kết nối một chạm) |
| Presentify, Mouseposé | Trình chiếu |
| Pulltube, Downie (phần tải) | Tải video qua yt-dlp |
| Sip, ColorSlurp | Chọn màu |
| Bartender (phần ẩn biểu tượng) | Bộ ẩn biểu tượng thanh menu |
| Session, Be Focused | Tab Pomodoro (chưa chặn app và web) |

## 2. Ứng viên mới, không cần extension

Khả thi: Dễ (một ngày hoặc ít hơn, API công khai, đã có phần lớn hạ tầng), Vừa (vài ngày, cần giao diện mới), Khó.

| App trả phí, giá tham khảo | Làm gì | Figghost hiện có | Cách làm trên macOS | Khả thi |
|---|---|---|---|---|
| Wispr Flow ($15 một tháng, $12 nếu trả năm), superwhisper ($8.49 một tháng, $249.99 trọn đời), MacWhisper (khoảng $59 đến 64 một lần, nguồn lệch) | Giữ một phím để nói, chữ hiện vào app đang dùng, có dọn câu bằng AI | Chưa. `research-raycast-2.5.0.md` để Dictation ở mục "Sau" | Nhận dạng trên máy bằng `SFSpeechRecognizer` (macOS 13) với `requiresOnDeviceRecognition`; sóng âm và trạng thái trên notch; dán bằng đường dán của clipboard. Dọn câu tùy chọn qua `claude -p` hoặc `codex exec` mà `core-agents` đã nhận ra, không giữ khóa API. Phải thử chất lượng tiếng Việt trước khi hứa | Vừa |
| LookAway 2 ($19 một lần theo một bài review) | Nhắc nghỉ mắt, chớp mắt, ngồi thẳng; tự hoãn khi đang họp, đang ghi màn hình, đang xem video toàn màn hình | Chưa (đã có trong `feature-gap.md`, lô 4) | Bộ đếm dùng lại `focus_timer`; tín hiệu "đang họp" lấy từ phần phát hiện camera và micro của Đảo trạng thái; "đang ghi" từ Capture; màn phủ kính toàn màn hình | Vừa |
| PopClip (trả phí; 2026.7 là bản nâng cấp trả phí đầu tiên cho người mua từ 2018 trở về trước) | Một thanh hành động tự hiện khi bôi đen chữ: sao chép, tìm, dịch, mở liên kết, và extension của người dùng | Một phần: Smart Select và Dịch cần phím tắt | Event tap đã có bắt chuột nhả sau khi kéo, đọc chữ chọn qua Accessibility như `read_selection`; bỏ qua ô mật khẩu (`AXSecureTextField`) và các app trong danh sách loại trừ | Vừa |
| Homerow (nguồn lệch: $29 một năm theo bài 2024, $50 trọn đời theo AlternativeTo) | Gắn nhãn chữ lên mọi phần tử bấm được để bấm bằng bàn phím | Chưa | Duyệt cây Accessibility của cửa sổ trước (`macos-sys/src/ax.rs`), vẽ nhãn trên một panel trong suốt. App Chromium và Electron cần bật `AXEnhancedUserInterface` hoặc `AXManualAccessibility`; cây lớn thì chậm | Khó |
| Hazel (chưa kiểm) | Quy tắc cho thư mục: tệp cũ hơn N ngày vào thùng rác, ảnh chụp màn hình về một thư mục, đổi tên theo mẫu | Chưa. Mole dọn theo yêu cầu, không theo quy tắc | FSEvents trên các thư mục người dùng chọn; logic khớp quy tắc là hàm thuần có kiểm thử. Chỉ chuyển vào Thùng rác, không xóa hẳn; khi bật quy tắc mới phải xem trước danh sách tệp sẽ bị tác động | Vừa |
| Clop (chưa kiểm) | Tự nén ảnh và video vừa sao chép hoặc vừa chụp | Một phần: nén có ở Drag wheel và menu của Shelf | Gọi phần nén sẵn có khi Clipboard nhận một ảnh mới, giữ bản gốc nếu bản nén không nhỏ hơn | Dễ |
| Moom, Rectangle Pro (chưa kiểm) | Lưu bố cục mọi cửa sổ và khôi phục, tự áp khi cắm hoặc rút màn hình | Một phần: đặt từng cửa sổ theo bố cục | `core-window` và `ax.rs` đã đặt được cửa sổ; thêm lưu danh sách (app, tiêu đề, khung) và thông báo đổi màn hình của `NSApplication` | Vừa |
| Witch (chưa kiểm) | Chuyển cửa sổ bằng phím, có ảnh thu nhỏ từng cửa sổ | Chưa | Ảnh thu nhỏ bằng ScreenCaptureKit, quyền Ghi màn hình Capture đã xin; danh sách cửa sổ từ `CGWindowListCopyWindowInfo` | Vừa |
| rcmd (chưa kiểm) | Command phải cộng chữ cái đầu để nhảy tới app | Chưa | Event tap đã có; có thể gộp vào Figghost Command của 2.5.0 thay vì làm riêng | Dễ |
| HazeOver (vài euro trên Mac App Store) | Làm tối mọi thứ phía sau cửa sổ đang dùng | Chưa | Một cửa sổ phủ mỗi màn hình, xếp ngay dưới cửa sổ trước của app khác bằng `orderWindow:relativeTo:` với số cửa sổ toàn cục. Cần thử việc xếp với cửa sổ của app khác trước khi hứa | Vừa |
| Mac Mouse Fix ($2.99 một lần), SteerMouse, BetterMouse | Gán nút phụ của chuột: lùi, tiến, giữ nút giữa và kéo để mở Mission Control hoặc đổi Space | Chưa: `core-mouse` chỉ có cuộn mượt | Event tap của cuộn mượt đã nhận sự kiện chuột; đổi Space bằng phím Control-mũi tên giả lập | Vừa |
| One Switch (chưa kiểm) | Một lưới công tắc: chế độ tối, ẩn biểu tượng màn hình nền, kết nối AirPods | Một phần: trang Tiện ích có giữ máy thức, tắt micro, khóa bàn phím | Chế độ tối qua System Events (cần quyền Automation); ẩn biểu tượng nền bằng khóa `CreateDesktop` của Finder; kết nối Bluetooth bằng `IOBluetoothDevice` | Dễ |
| iStat Menus ($11.99) | Số liệu ngay trên thanh menu, không cần mở gì | Một phần: tab System chỉ hiện khi mở notch | Một `NSStatusItem` hiện CPU, bộ nhớ hoặc mạng, đọc bằng `sysinfo.rs` đã có, mỗi giây một lần chỉ khi bật | Dễ |
| Bartender 6 hoặc 7 (khoảng $20 đến 25), Barbee | Hiện biểu tượng khi nó đổi, tìm trong thanh menu, quy tắc hiện theo sự kiện | Một phần: ẩn và hiện | Đọc biểu tượng của app khác cần Accessibility và ảnh chụp thanh menu; mong manh qua mỗi bản macOS | Khó |
| Hand Mirror (bản Plus trả phí) | Gương camera từ thanh menu | Chưa (đã có trong `feature-gap.md`, lô 4) | AVCaptureSession trong notch | Vừa |

## 3. Nhóm app có extension trình duyệt

| App | Extension làm gì | App và extension nói chuyện thế nào | Figghost làm được gì |
|---|---|---|---|
| Downie, Pulltube | Nút trên thanh công cụ, mục chuột phải cho liên kết, phím tắt; gửi địa chỉ trang sang app để tải. Bản Safari nằm sẵn trong app | Extension rất đơn giản, không chạy nền | Nút "Tải bằng Figghost" cho trang Tải video (yt-dlp) |
| Choosy, OpenIn, Velja | App làm trình duyệt mặc định, chọn trình duyệt theo quy tắc; extension chuyển trang đang xem sang trình duyệt khác | Native messaging (extension Chrome của Choosy xin quyền `nativeMessaging`) | Bộ chọn trình duyệt: Figghost khai nhận `http` và `https`, mở theo quy tắc (link Meet vào Chrome, link Zoom vào app Zoom), hỏi bằng một bảng nhỏ trên notch khi không có quy tắc. Dùng chung việc khai `CFBundleURLTypes` với deeplink `figghost://` của 2.5.0 |
| Timing | Thời gian theo từng trang web, mọi trình duyệt | Không dùng extension: AppleScript của Safari và Chrome để đọc tab đang mở, cộng quyền Accessibility. Flexible Timers thì dùng extension | Screen Time theo tên miền, đọc bằng AppleScript. Firefox không có AppleScript cho tab nên cần extension |
| Freedom, Cold Turkey, Focus, 1Focus, Opal | Chặn trang và app theo phiên, có chế độ khóa không dừng được | Chặn ở trình duyệt hoặc ở tầng mạng | Chặn trang web trong phiên Pomodoro: extension dùng `declarativeNetRequest` trên Chromium và Firefox; Safari chỉ đóng tab qua AppleScript. Chặn ở tầng mạng vẫn không làm (cần `NEFilterProvider`) |
| Raycast Companion | Đọc nội dung tab đang mở dạng HTML, chữ hoặc Markdown; tóm tắt trang; chèn tab vào lệnh AI | Extension nói với app Raycast trên máy | Công cụ MCP `browser_tab` để agent AI (Claude Code, Codex) đọc trang người dùng đang mở, cùng ổ cắm và định dạng với `core-clipboard/src/mcp.rs`; mỗi lần đọc phải được người dùng cho phép |
| Immersive Translate (Pro $9.99 một tháng, $79.99 một năm) | Dịch song ngữ cả trang, giữ bố cục | Extension tự gọi dịch vụ dịch | Khó: trang Dịch của Figghost gửi chữ tới MyMemory, có giới hạn lượt và không hợp gửi cả trang; dịch trên máy bằng Translation framework của Apple chỉ có API Swift, phải viết cầu nối |
| Hookmark | Liên kết tới tài liệu và trang đang mở | AppleScript và extension | "Sao chép liên kết Markdown của tab đang mở" và ghi nguồn URL cho mục Clipboard sao chép từ trình duyệt, đều làm được bằng AppleScript |
| Notchy (đổi tốc độ video trong trình duyệt) | Đổi tốc độ phát của video trong tab | Không rõ | `feature-gap.md` đánh dấu Khó. Có extension thì thành Vừa: tab Media gửi lệnh tốc độ và tua tới content script trên thẻ `video` của tab đang phát |

Kết luận của nhóm này: nhiều việc không cần extension. Đọc tab, đóng tab, lấy URL làm được bằng AppleScript cho Safari và họ Chromium, đúng cách Timing làm. Gửi một lệnh một chiều (tải video này, mở trang này) làm được bằng deeplink `figghost://` đã đề xuất cho 2.5.0. Extension chỉ cần cho bốn việc: chặn trang web thật, điều khiển video trong trang, đưa nội dung trang cho MCP, và Firefox.

## 4. Kiến trúc đề xuất: một extension Figghost Companion

1. Một extension Manifest V3, một bộ mã cho Chromium (Chrome, Edge, Brave, Vivaldi, Opera, Arc, Chromium) và Firefox, khác nhau ở manifest.
2. Cầu nối là native messaging, không mở cổng HTTP trên localhost: một trang web bất kỳ gọi được localhost, còn host native messaging chỉ extension có tên trong manifest mới gọi được.
3. Host là chính binary Figghost chạy với một cờ riêng (như các chương trình hook của agent), đọc tin dạng độ dài cộng JSON trên stdin, chuyển tiếp vào ổ cắm Unix mà app đang nghe, theo cách `core-agents/src/server.rs` làm. Dùng `connectNative` để giữ một tiến trình suốt phiên; `sendNativeMessage` tạo một tiến trình mới cho mỗi tin.
4. Cài host: ghi tệp manifest vào thư mục `NativeMessagingHosts` của từng trình duyệt đã cài, theo đúng cách `core-agents/src/install.rs` ghi hook: chỉ thêm tệp của mình, gỡ được từ trang Cài đặt. Đường dẫn trên macOS:
   - Chrome: `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/` (tài liệu Chrome)
   - Chromium: `~/Library/Application Support/Chromium/NativeMessagingHosts/` (tài liệu Chrome)
   - Firefox: `~/Library/Application Support/Mozilla/NativeMessagingHosts/` (MDN)
   - Edge, Brave, Vivaldi, Opera: thư mục của từng trình duyệt trong `~/Library/Application Support` (`Microsoft Edge`, `BraveSoftware/Brave-Browser`, `Vivaldi`, `com.operasoftware.Opera`, đều có trên máy này), cộng `NativeMessagingHosts`. Chưa có tài liệu chính thức trong nguồn đã đọc, phải thử từng trình duyệt
5. Manifest của host có `name` (chỉ chữ thường, số, gạch dưới, dấu chấm), `description`, `path` (tuyệt đối trên macOS), `type` là `stdio`, và `allowed_origins` liệt kê `chrome-extension://<ID>/` không có ký tự đại diện. Vì thế ID extension phải cố định: đặt khóa `key` trong manifest của extension khi nạp ở chế độ nhà phát triển. Firefox dùng `allowed_extensions` với ID của add-on.
6. Giới hạn kích thước: host gửi tối đa 1 MB một tin, trình duyệt gửi tối đa 64 MiB. Nội dung trang cho MCP phải cắt bớt hoặc chia nhỏ.
7. Ranh giới tin cậy: mọi tin từ extension là dữ liệu không tin cậy, vì content script chạy cùng trang web. Host chỉ nhận một danh sách lệnh cố định, kiểm kích thước, không chạy lệnh shell từ tin nhắn. URL tải video chỉ nhận `http` và `https`, truyền cho yt-dlp như một đối số sau `--` để không thành cờ. Nội dung trang gửi cho agent AI là dữ liệu, không phải chỉ dẫn.
8. Phân phối:
   - Chromium: Chrome Web Store cần tài khoản nhà phát triển (phí đăng ký một lần) và qua duyệt. Bản beta có thể hướng dẫn nạp thư mục ở chế độ nhà phát triển.
   - Firefox: extension phải được ký trên addons.mozilla.org, có thể để không công khai.
   - Safari: Safari Web Extension phải nằm trong app. Theo diễn đàn Apple, từ Safari 18.4 bản ký Developer ID và đã notarize mới nạp được mà không cần bật "Allow Unsigned Extensions" (cài đặt này tự tắt mỗi lần mở Safari); chưa thấy tài liệu chính thức. Figghost ký ad hoc nên với Safari chỉ dùng AppleScript: đọc URL, đóng tab; chạy JavaScript trong tab cần người dùng bật "Allow JavaScript from Apple Events" trong menu Develop.
9. Chỗ để mã, theo cấu trúc hiện có: `crates/core-browser` (giao thức, kiểm tin, quy tắc chặn và quy tắc chọn trình duyệt, có kiểm thử), phần cài manifest trong `macos-sys` hoặc cạnh `core-agents/src/install.rs`, extension trong `extensions/companion/`, trang Cài đặt `web/features/browser.js` liệt kê trình duyệt đã cài, trạng thái host của từng cái, nút cài và gỡ.

## 5. Thứ tự đề xuất

1. Lô A, không cần extension, ít công: số liệu trên thanh menu (iStat Menus), công tắc nhanh trong Tiện ích (One Switch), tự nén ảnh vừa sao chép (Clop), Screen Time theo tên miền qua AppleScript (Timing), sao chép liên kết Markdown của tab (Hookmark). Screen Time hiện hứa "không nhìn tiêu đề cửa sổ, không nhìn chữ"; theo dõi trang web phải là một công tắc riêng, chỉ lưu tên miền, không lưu đường dẫn đầy đủ, và bỏ qua cửa sổ ẩn danh.
   Trạng thái ngày 29/10/2026: đã làm bốn mục.
   - Số liệu trên thanh menu: trang Hệ thống, nhóm Thanh menu.
   - Công tắc giao diện tối và ẩn biểu tượng màn hình nền: trang Tiện ích; chưa có nút kết nối AirPods.
   - Screen Time theo trang web: công tắc "Tính cả theo trang web", nút "Trang web" trên tab.
   - Sao chép liên kết của tab: trang Tiện ích, phím Control-Option-Command-L, ba dạng viết.

   Phần đọc tab dùng chung nằm ở `macos-sys/src/browser.rs`. Bỏ tự nén ảnh sao chép: tính năng này đổi thứ người dùng dán mà họ không thấy, móc ghi clipboard (`set_on_copied`) đã do mascot dùng, và nén theo yêu cầu đã có ở Shelf và Drag wheel.
2. Lô B, giao diện mới: đọc chính tả (Wispr Flow), nhắc nghỉ mắt (LookAway), thanh hành động khi bôi đen (PopClip), nút phụ của chuột (Mac Mouse Fix), bộ chọn trình duyệt (Choosy, làm cùng deeplink 2.5.0), bố cục cửa sổ (Moom), quy tắc thư mục (Hazel).
3. Lô C, extension Figghost Companion cho Chromium và Firefox: nút tải video, chặn trang web trong phiên Pomodoro, điều khiển tốc độ và tua video từ tab Media, công cụ MCP `browser_tab`.
   Trạng thái ngày 29/10/2026:
   - Lô B: đã làm nhắc nghỉ mắt (trang Nghỉ mắt). Đọc chính tả mới làm thử: tiếng Việt thuần nhận dạng tốt với giọng tổng hợp, câu trộn tiếng Anh thì sai; còn chờ thử giọng thật.
   - Lô C: đã làm phần đầu cho Chrome, Edge, Brave, Vivaldi và Arc (trang Trình duyệt): cầu nối native messaging, chặn trang trong phiên làm việc Pomodoro, nút và menu tải video. Còn lại: điều khiển video, công cụ MCP, Firefox.
4. Sau: nhãn bàn phím cho phần tử (Homerow), chuyển cửa sổ có ảnh (Witch), làm tối nền (HazeOver), gương camera (Hand Mirror).

## 6. Không làm, và lý do

| Tính năng | Lý do |
|---|---|
| Giới hạn mức sạc, Sailing Mode, Heat Protection (AlDente Pro, từ $14.49 một năm trên Setapp) | Cần một tiến trình chạy quyền root ghi SMC, cùng lý do không chỉnh tốc độ quạt trong `feature-gap.md` |
| Sửa ngữ pháp, chính tả (Grammarly, LanguageTool) | Cần máy chủ hoặc mô hình riêng |
| Tường lửa theo app, chặn mạng theo app (Little Snitch, TripMode) | Cần Network Extension, phải có team trả phí và provisioning profile |
| Cử chỉ trackpad tùy ý (BetterTouchTool) | Cần framework MultitouchSupport riêng tư. Hyper key đã có trong kế hoạch 2.6 |
| Màn hình ảo HiDPI (BetterDisplay Pro) | Cần `CGVirtualDisplay` riêng tư |
| Safari extension | Cần ký Developer ID và notarize; thay bằng AppleScript cho tới khi có |
| Dịch song ngữ cả trang | Dịch vụ dịch hiện tại có giới hạn lượt và sẽ phải nhận cả trang; dịch trên máy cần cầu nối Swift |
| Phiên khóa không dừng được kiểu Cold Turkey | Người dùng chỉ cần gỡ extension hoặc thoát Figghost là hết; hứa "không dừng được" là hứa sai. Chỉ làm chặn có nút dừng |

## 7. Câu hỏi mở cho người quyết định

1. Có định đăng ký Apple Developer Program để ký Developer ID không? Điều này quyết định Safari extension, notarization và cả WidgetKit đã ghi trong `feature-gap.md`.
2. Extension phát hành trên Chrome Web Store và addons.mozilla.org, hay chỉ nạp ở chế độ nhà phát triển cho bản beta?
3. Đọc chính tả chỉ nhận dạng trên máy, hay cho phép gửi âm thanh ra dịch vụ ngoài khi tiếng Việt trên máy không đủ tốt?
4. Lô nào làm trước: lô A (nhanh, không cần extension) hay đi thẳng vào extension?

## Nguồn

- Timing, "Best Mac menu bar apps": https://timingapp.com/blog/best-mac-menu-bar-apps/
- SupaSidebar, "Best Mac menu bar apps 2026": https://supasidebar.com/blog/best-mac-menu-bar-apps-2026
- Timing, theo dõi trình duyệt: https://timingapp.com/browser-time-tracking và FAQ: https://timingapp.com/help/faq
- Timing, "Best distraction blocker for Mac": https://timingapp.com/blog/best-distraction-blocker-mac/
- Freedom cho Mac: https://freedom.to/freedom-for-mac
- Raycast, Browser Extension API: https://developers.raycast.com/api-reference/browser-extension
- Raycast Companion trên Chrome Web Store: https://chromewebstore.google.com/detail/fgacdjnoljjfikkadhogeofgjoglooma
- Downie, Browser Extensions: https://site2021.charliemonroe.net/help/downie/extensions.html
- Choosy: https://choosy.app/ và https://addons.mozilla.org/firefox/addon/choosy/
- Chrome, Native messaging: https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging
- MDN, Native manifests: https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_manifests
- Diễn đàn Apple về Safari extension ký Developer ID: https://developer.apple.com/forums/thread/782005 và https://developer.apple.com/forums/thread/700059
- So sánh đọc chính tả: https://spokenly.app/blog/wispr-flow-vs-superwhisper-vs-macwhisper và https://blog.buildbetter.ai/best-wispr-flow-alternatives-in-2026-ai-dictation-for-mac-power-users/
- PopClip 2026.7: https://forum.popclip.app/t/popclip-2026-7-released/3849
- Homerow: https://curtismchale.ca/2024/04/30/use-your-mac-without-a-mouse-with-homerow và https://alternativeto.net/software/homerow/about
- LookAway 2: https://makerstack.co/reviews/lookaway-review/
- AlDente Pro: https://setapp.com/apps/aldente-pro
- BetterDisplay: https://github.com/waydabber/BetterDisplay
- Mac Mouse Fix: https://noahnuebling.gumroad.com/l/mmfinappusd
- HazeOver: https://www.appgefahren.de/hazeover-mac-app-lenkt-den-fokus-auf-das-aktive-fenster-167990.html
- Immersive Translate: https://apps.apple.com/app/immersive-translate/id6447957425
- Setapp: https://setapp.com/app-reviews/setapp-pricing-explained
