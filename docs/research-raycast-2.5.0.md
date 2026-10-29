# Raycast và bản 2.5.0 của Figghost

Nghiên cứu Raycast (bản 2.x) để chọn những gì đưa vào Figghost, với 2.5.0 là bản mở màn. Tài liệu này nối tiếp `docs/research-raycast-powertoys.md` (16/10/2026) và `docs/feature-gap.md` (20/10/2026), đối chiếu lại với mã ở bản 2.1.5.

## Nguồn

Trang changelog và trang "New in v2" của Raycast, manual Raycast (Window Management, Focus, AI Extensions), blog "A Technical Deep Dive Into the New Raycast", đọc ngày 23/10/2026. Mô tả lấy từ lời của Raycast, chưa dùng thử. Nguyên tắc của `feature-gap.md` vẫn giữ: viết lại độc lập từ mô tả công khai, không lấy mã, hình hay tên sản phẩm.

## Raycast hiện có gì

| Nhóm | Tính năng |
|---|---|
| Root search | Một ô tìm duy nhất mở bằng phím tắt toàn cục: app, lệnh, tệp (2.x gộp File Search vào root), cài đặt, kết quả máy tính ngay khi gõ. Xếp hạng theo tần suất và độ gần đây (frecency), mục yêu thích ghim trên đầu, chế độ Compact |
| Thao tác bằng phím | Action Panel (Cmd-K) trên mọi mục, gán phím tắt ngay trong Action Panel, alias chữ cho từng lệnh, phím tắt riêng cho từng lệnh, Fallback Commands khi không có kết quả, điều hướng theo ngăn xếp (Enter vào trong, Esc ra ngoài) |
| Hyper key | Biến Caps Lock thành Cmd-Ctrl-Opt-Shift, nhấn riêng vẫn là Caps Lock hoặc Escape |
| Quicklinks | Liên kết có chỗ trống `{query}`, mở bằng trình duyệt hoặc app chọn trước |
| Snippets | Mở rộng từ khóa, có placeholder ngày giờ, clipboard, con trỏ |
| Clipboard History | Lịch sử chữ, ảnh, tệp, tìm, ghim |
| Calculator | Biểu thức, đơn vị, tiền tệ, múi giờ, ngày tháng bằng ngôn ngữ tự nhiên |
| Window Management | Nửa, ba phần, bốn phần, sáu phần, nhiều màn hình, chuyển desktop 1-9, kích thước và tọa độ chính xác (`1920x1080`, `100,100`) |
| Emoji và ký hiệu | Tìm emoji theo từ khóa, chèn vào app đang dùng |
| Notes | Ghi chú Markdown nổi trên mọi cửa sổ, nhiều ghi chú, đồng bộ đám mây |
| Focus | Phiên tập trung chặn hoặc chỉ cho phép một số app và trang web, nhóm chặn dùng lại, thanh đếm ngược nổi, tự dừng khi máy ngủ |
| AI | Quick AI từ ô tìm, chat, AI Extensions gọi bằng `@`, agent nhiều bước, MCP |
| Mở rộng | Store extension (React và Node), Script Commands (một tệp shell hoặc Python có chú thích ở đầu), Deeplinks `raycast://` |
| Khác | Dictation, Translator, My Schedule, System commands (ngủ, khóa, đổ thùng rác, đẩy ổ), quản lý tiến trình |

## Figghost đã có gì trong số đó

| Tính năng Raycast | Figghost 2.1.5 |
|---|---|
| Clipboard History | Có, sâu hơn (Copy Stack, boards, loại bỏ dữ liệu nhạy cảm, MCP) |
| Snippets | Có (trang Gõ tắt) |
| Window Management | Có (snap từ notch, preset, lưới, theo màn hình, phím tắt). Chưa có kích thước chính xác, chưa có lệnh chuyển desktop |
| Calculator | Một phần: `core-smart` đổi đơn vị, tiền, múi giờ cho chữ được chọn. Chưa có bộ tính biểu thức `2*(3+4)` |
| Notes | Có (tab Notes, tệp văn bản thuần). Chưa có cửa sổ nổi |
| Translator, My Schedule | Có (Dịch, tab Calendar) |
| Keep awake, tắt micro | Có (Tiện ích) |
| Launcher | Một phần: lưới 12 mục ghim trong notch. Không có tìm kiếm |
| Tìm cài đặt | Có trong cửa sổ Cài đặt (Cmd-K), không gọi được từ ngoài |
| MCP | Một phần: `core-clipboard/src/mcp.rs` chỉ có `clipboard_search` và `clipboard_get` |
| Root search, Action Panel, alias, frecency, fallback | Chưa |
| Deeplinks | Chưa: `scripts/bundle.sh` không khai `CFBundleURLTypes` (chuỗi `figghost://localhost` trong `crates/desktop` là giao thức nội bộ của webview, không phải URL scheme của hệ thống) |
| Quicklinks, Emoji, System commands, quản lý tiến trình | Chưa |
| Hyper key | Chưa (đã có event tap ở `macos-sys/src/event_tap.rs` và bộ nhận phím bổ trợ) |
| Focus chặn app và web | Chưa (Pomodoro chỉ đếm giờ) |
| Script Commands, Dictation, Quick AI | Chưa |

Kết luận: phần "nội dung" Figghost đã có gần đủ (clipboard, snippet, ghi chú, snap, chuyển đổi, dịch, lịch). Thứ thiếu là **lớp truy cập**: một ô gõ duy nhất để tới mọi thứ đó bằng bàn phím. Đó chính là trái tim của Raycast, và là việc đáng làm nhất cho 2.5.0.

## Đề xuất cho 2.5.0: Figghost Command

Một ô tìm kiếm nổi (Liquid Glass, mọc ra từ notch hoặc giữa màn hình), mở bằng một phím tắt toàn cục (mặc định Option-Space, đổi được, từ chối nếu trùng). Tắt theo mặc định như mọi tính năng mới.

### Phạm vi 2.5.0

1. **Nguồn kết quả**, mỗi nguồn là một provider trả về danh sách mục có điểm:
   - App: quét `/Applications`, `/System/Applications`, `/System/Applications/Utilities`, `~/Applications`, mở bằng `NSWorkspace`. Không cần quyền.
   - Lệnh của Figghost: bật tắt từng tính năng, mở từng trang Cài đặt (dùng lại chỉ mục tìm của cửa sổ Cài đặt), từng bố cục snap cho cửa sổ đang trước, chụp màn hình, chọn màu, giữ máy thức, tắt micro, khóa bàn phím, snooze notch, bắt đầu Pomodoro.
   - Clipboard, Snippets, Notes: dùng lại kho SQLite và `core-clipboard::search::fuzzy_score`.
   - Máy tính và chuyển đổi: `core-smart` đã có đơn vị, tiền, múi giờ; thêm một bộ tính biểu thức nhỏ (cộng trừ nhân chia, lũy thừa, ngoặc, phần trăm) trong `core-smart`, có kiểm thử, theo đúng `NumberStyle` Anh và Việt.
   - Quicklinks: tên, URL có `{query}`, mở bằng trình duyệt mặc định. Cài sẵn vài mục (Google, GitHub, YouTube, Từ điển), sửa được ở trang Cài đặt.
   - Fallback: khi không có kết quả, đưa ra "Tìm trên web", "Dịch", "Lưu thành ghi chú", "Hỏi AI" (xem mục sau).
2. **Thao tác bằng phím**: lên xuống, Enter chạy hành động chính, Cmd-K mở danh sách hành động phụ của mục (với clipboard: dán, dán chữ thuần, ghim, xóa; với app: mở, hiện trong Finder, thoát), Cmd-1..9 chạy nhanh, Esc lùi một cấp rồi đóng.
3. **Xếp hạng**: điểm khớp mờ cộng frecency lưu trong một bảng SQLite nhỏ; mục yêu thích ghim trên đầu khi ô trống.
4. **Alias**: gán chữ ngắn cho bất kỳ mục nào (ví dụ `cl` mở clipboard, `gh` là quicklink GitHub).
5. **Deeplinks `figghost://`**: khai `CFBundleURLTypes` trong `scripts/bundle.sh`, xử lý `application:openURLs:`. Ví dụ `figghost://command/snap?layout=left-half`, `figghost://toggle/awake`, `figghost://settings/clipboard`. Mỗi lệnh của mục 1 có một deeplink, nên Apple Shortcuts, Raycast, Alfred, script shell đều điều khiển được Figghost. Đầu vào từ URL là ranh giới tin cậy: chỉ nhận danh sách lệnh cố định, không chạy lệnh shell từ URL.

### Vì sao chọn phạm vi này

- Gần như toàn bộ dữ liệu và hành động đã có sẵn; 2.5.0 chủ yếu là một cửa sổ, một bộ xếp hạng và một bảng lệnh.
- Biến mỗi tính năng hiện có thành một dòng gõ được, nên giá trị tăng theo số tính năng đã có (khoảng 40 trang).
- Deeplink rẻ (một khai báo và một bộ phân tích có kiểm thử) và mở cửa cho mọi công cụ tự động hóa.

### Cách chia mã (theo cấu trúc hiện có)

- `crates/core-command` (mới, `forbid(unsafe_code)`): mô hình mục, provider trait, xếp hạng và frecency, alias, quicklink, bộ phân tích deeplink, bộ tính biểu thức (hoặc đặt bộ tính trong `core-smart`). Toàn bộ có kiểm thử đơn vị.
- `crates/macos-sys/src/command_ui.rs`: panel nổi, ô nhập, danh sách, Action Panel, theo kiểu `launcher_ui.rs` và `style.rs`.
- `crates/app-ui/src/features/command/`: `mod.rs`, `logic.rs`, `settings.rs`, `text.rs` như các feature khác.
- `crates/app-ui/web/features/command.js`, `.css`, `.fixture.json`: trang Cài đặt (phím tắt, nguồn nào bật, quicklinks, alias).
- `scripts/check-command.sh`, mục trong `docs/configuration.md`, `docs/verification.md`, chuỗi hai ngôn ngữ.

### Việc cần kiểm trước khi làm

1. Option-Space trùng với gì trên máy người dùng (Spotlight dùng Cmd-Space, Raycast hay dùng Option-Space). Nếu Raycast đang chạy thì gợi ý phím khác.
2. Panel nhận bàn phím mà không đẩy app đang dùng ra sau (`NSPanel` kiểu non-activating, giống clipboard popup hiện tại) để dán vào đúng app.
3. Bộ gõ tiếng Việt của chính Figghost phải gõ được trong ô tìm (ô nằm trong process của Figghost; kiểm `core-typing` có bỏ qua app của mình không).
4. Thời gian quét app lần đầu và cập nhật khi cài app mới (`NSMetadataQuery` theo dõi hoặc quét lại khi mở panel).

## Sau 2.5.0: các bản 2.5.x và 2.6

Xếp theo giá trị trên công sức. Mỗi mục thêm một provider hoặc một hành động vào Figghost Command, nên không cần giao diện mới.

| Bản | Tính năng | Ghi chú kỹ thuật |
|---|---|---|
| 2.5.x | Tìm tệp trong ô Command | `NSMetadataQuery` trên chỉ mục Spotlight, không cần Full Disk Access cho tệp của người dùng |
| 2.5.x | Emoji và ký hiệu, có từ khóa tiếng Việt | Bảng emoji Unicode nhúng sẵn; chèn bằng đường dán của clipboard |
| 2.5.x | Lệnh hệ thống và tiến trình: ngủ, khóa, đổ thùng rác, đẩy ổ, danh sách app theo CPU và bộ nhớ, thoát hoặc buộc thoát | `libproc`; `sysinfo.rs` đã đọc số liệu hệ thống |
| 2.5.x | Advanced Paste: chữ thuần, bỏ tham số theo dõi khỏi URL, JSON sang YAML hoặc CSV, base64, đổi hoa thường | Hàm thuần trong `core-clipboard`, hiện trong Action Panel của mục clipboard. Hiện chỉ có `paste_plain` |
| 2.5.x | Lệnh cửa sổ bằng bàn phím: kích thước chính xác, căn giữa, chuyển sang màn hình kế, khôi phục | `ax.rs` và `core-window` đã đặt được cửa sổ |
| 2.5.x | Phím tắt riêng cho từng mục của Command | Dùng lại `macos-sys/src/hotkey.rs` và kiểm tra trùng của trang Phím tắt |
| 2.6 | Hyper key | Event tap đã có; Caps Lock nhấn riêng giữ chức năng cũ hoặc thành Escape. Phải không đụng phím chuyển Anh Việt của trang Gõ tiếng Việt |
| 2.6 | Script Commands | Thư mục do người dùng chọn, mỗi tệp shell có chú thích `@figghost.title`, `@figghost.mode`; chạy bằng `process_group(0)` và `process::kill_group` khi hủy. Là đường mở rộng thay cho store extension |
| 2.6 | Focus: chặn app gây xao nhãng trong phiên Pomodoro | Ẩn hoặc thoát app trong danh sách khi nó ra trước (`NSWorkspace` thông báo app kích hoạt). Chặn web chỉ làm được bằng cách đọc tab đang mở của Safari và Chrome qua AppleScript (cần quyền Automation) rồi đóng tab, không phải chặn thật ở mạng. Kiểm cách Raycast Focus làm trước khi hứa |
| 2.6 | Ghi chú nổi | Mở một ghi chú của tab Notes trong panel luôn trên cùng |
| 2.6 | Quick AI qua CLI đã cài | Gửi câu hỏi từ ô Command tới `claude -p` hoặc `codex exec` mà `core-agents` đã nhận ra; không giữ khóa API |
| 2.6 | Thêm công cụ MCP: notes, snippets, quicklinks, chạy lệnh Figghost | Cùng ổ cắm và định dạng với `core-clipboard/src/mcp.rs`; mỗi công cụ ghi phải hỏi người dùng |
| Sau | Dictation | `SFSpeechRecognizer` trên máy (macOS 13), cần quyền Speech Recognition |

## Không làm

| Tính năng | Lý do |
|---|---|
| Store extension chạy React và Node | Kéo theo một runtime JavaScript, đi ngược mục tiêu khoảng 25 MB và ứng dụng thuần native. Script Commands và deeplinks đủ làm đường mở rộng |
| Đồng bộ đám mây, tài khoản | Cần máy chủ hoặc entitlement iCloud mà app ký ad hoc không có |
| AI có trả phí, chọn model | Ngoài phạm vi; dùng CLI người dùng đã đăng nhập thay thế |
| Chặn web ở tầng mạng | `NEFilterProvider` cần team trả phí và provisioning profile |
| Chuyển desktop 1-9, đổi tên desktop | Không có API công khai cho Spaces |

## Câu hỏi mở cho người quyết định

1. Ô Command mọc từ notch (đồng bộ với bản sắc Figghost) hay nằm giữa màn hình như Raycast và Spotlight?
2. Phím tắt mặc định: Option-Space, Cmd-Shift-Space, hay nhấn đúp một phím bổ trợ?
3. Lưới Launcher 12 mục có giữ riêng, hay thành "mục yêu thích" của Command để chỉ còn một danh sách ghim?
4. 2.5.0 có gồm Advanced Paste và lệnh hệ thống không, hay giữ thật gọn chỉ Command, Quicklinks và deeplinks?
