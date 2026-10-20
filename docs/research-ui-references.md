# Tham khảo giao diện: học gì để Figghost đẹp hơn và dễ nhận ra hơn

Mục đích: tìm các phần mềm cùng kiểu với Figghost (tiện ích macOS dùng Liquid Glass, có notch, cửa sổ cài đặt nhiều trang) để rút ra cách làm giao diện, thanh điều hướng và các trang đẹp hơn, khác biệt hơn. Tài liệu chỉ nghiên cứu và đề xuất, chưa sửa mã nguồn. "Style này" được hiểu là style hiện tại của Figghost: kính, notch đen, con ma và các nhân vật pixel.

Đọc nguồn ngày 20 tháng 10 năm 2026, trên máy chạy macOS 27.

## Độ tin cậy của nguồn

- Đọc trực tiếp từ nguồn gốc: Apple HIG (Sidebars, Materials, Settings), MacRumors, 9to5Mac, blog Raycast, hai bài của Linear, trang Apple Design Awards.
- Lời của chính hãng bán hàng, có thiên vị: macnotch.io (do nhà làm MacNotch viết), getseam.app (nhà làm Seam), notchy.dev. Danh sách tính năng, giá và nhận xét về đối thủ lấy từ các trang này.
- Đã xem ảnh: cài đặt Raycast v2 (một ảnh cắt từ trang quảng bá, không phải cả cửa sổ); cài đặt, notch và panel tray của Figghost (ảnh trong `docs/images` và ảnh render bằng `settings_preview`, `tray_preview`).
- Chưa kiểm chứng: cấu trúc cửa sổ cài đặt của CleanShot X, iStat Menus, Rectangle Pro, Bartender, NotchNook, Alcove, MacNotch. Công cụ tìm kiếm có trả về mô tả nhưng không kèm nguồn đáng tin hay ảnh, nên không dùng.

## 1. Hiện trạng của Figghost

Đo lại ngày 20/10/2026 trên bản 1.4.2. Bản đầu của tài liệu này dựa trên 1.2.0; `main` được kéo về lúc 09:28 cùng ngày và đã đổi phần điều hướng, nên các điểm về sidebar bên dưới là của 1.4.2. Cửa sổ mặc định 1000 x 700, tối thiểu 820 x 560 (`app-ui/src/settings/mod.rs`).

1. Sidebar là cột kính sát mép cửa sổ, rộng 236 px, có ô tìm kiếm và năm nhóm có tên (General, Mouse & Cursor, Notch & Island, Clipboard & Tools, Window & Keys). Mỗi nhóm có ô icon vuông bo góc màu riêng; các trang con là thanh tab dạng viên dưới tiêu đề (`CATEGORIES` và `renderSubtabs` trong `app.js`). Tức là hai điều đã đề xuất ở bản đầu (nhãn nhóm, sidebar sát mép) đã có từ 1.4.0 và 1.4.2.
2. Mỗi trang vẫn cùng một khuôn: tiêu đề lớn, thanh tab con, rồi chồng thẻ chứa các hàng. Dòng mô tả của trang bị ẩn (`.page-sub { display: none }`). Chỉ General và About có hero.
3. Kính ở đây là CSS trên một canvas bán trong suốt (`--canvas` là `rgba(...)` nên lớp kính thật của cửa sổ lộ ra qua nó): `.card` và `#sidebar` làm mờ 28 đến 40 px, bán kính thẻ 16 px. Điểm này tôi từng suy ngược lại ở bản đầu (nói canvas đặc che mất kính thật); đọc lại CSS thì không đúng.
4. Cửa sổ vẫn luôn mở ở trang General (`go('general')` ở trạng thái đầu tiên); mỗi nhóm nhớ trang con cuối (`cat.lastPage`) nhưng chỉ trong phiên.
5. Hai bầu không khí: notch, icon và ảnh hero là đen với ánh sáng tím xanh; cửa sổ cài đặt là xanh xám nhạt. Con ma và sáu nhân vật pixel chỉ có ở hero General, trang Cursor, hiệu ứng hello và linh vật clipboard.
6. Notch: thanh tab chỉ có icon, trang là danh sách trên nền đen thuần, màu trạng thái nhất quán. Panel tray là danh sách công tắc đúng chức năng, ít dấu ấn riêng.

Nên giữ: token CSS ở đầu `app.css`; tương phản đã đo; fallback cho Reduce transparency, Increase contrast, Reduce motion; quy tắc một màu không mang hai nghĩa; tìm kiếm theo nội dung từng hàng có tô sáng; song ngữ Anh và Việt.

Đã làm sau tài liệu này (20/10/2026, nhánh `feature/ada-style-and-islands`): lớp "Ghost world" (mục 5.1, 5.3 và 5.5 bên dưới, mô tả ở [design.md](design.md)) và lô tính năng đầu tiên ([feature-gap.md](feature-gap.md)).

## 2. Apple yêu cầu gì, và vừa đổi gì

| Quy tắc | Nguồn | Với Figghost |
|---|---|---|
| Liquid Glass là lớp chức năng cho điều hướng và điều khiển, nổi trên lớp nội dung. "Don't use Liquid Glass in the content layer": nền và nội dung dùng vật liệu chuẩn; dùng kính tiết kiệm. | HIG Materials, cập nhật 9/9/2025 | Thẻ nội dung đang là kính. Nên đổi thành bề mặt đặc, giữ kính cho sidebar và thanh tiêu đề. |
| Sidebar: nhóm có tiêu đề ngắn, tối đa hai cấp, dùng disclosure khi nhiều mục; cho người dùng tùy biến nội dung sidebar; cho ẩn sidebar; không đặt thông tin hay hành động quan trọng ở đáy. | HIG Sidebars | Thêm nhãn nhóm và thu gọn; đưa trạng thái quyền lên đầu; cho ẩn trang của tính năng đang tắt. |
| Icon sidebar mặc định theo màu nhấn hệ thống; người dùng đổi màu nhấn thì mong mọi icon đổi theo. Màu cố định chỉ dùng tiết kiệm và phải có mục đích. | HIG Sidebars, đổi ngày 8/6/2026 | Không tô cầu vồng cho từng icon. Màu cố định chỉ nên gắn với nhóm hoặc trạng thái, hoặc đặt ở tiêu đề trang. |
| Cửa sổ Settings: ít cài đặt; tiêu đề cửa sổ theo trang đang xem; mở lại trang xem gần nhất; thanh điều hướng luôn thấy và báo trang hiện tại. | HIG Settings | Hiện luôn mở ở General và tiêu đề cửa sổ cố định "Figghost". |
| macOS 27 "Golden Gate": thanh trượt Liquid Glass từ trong suốt đến đục, mặc định ở giữa, áp cho sidebar, toolbar và nút trên toolbar. Sidebar kéo sát mép cửa sổ, khúc xạ chạy tiếp bên dưới. Icon sidebar giữ màu. | MacRumors 8/6/2026; 9to5Mac 22/9/2026 | Sidebar 1.4.2 đã sát mép và có icon màu. Còn lại là việc lớp kính của sidebar có theo thanh trượt của người dùng hay không (xem 5.1). |

## 3. Phần mềm tham khảo

### Nhóm A: cùng loại (notch, kính, hộp công cụ)

| Phần mềm | Điểm đáng học | Không nên bắt chước | Nguồn |
|---|---|---|---|
| MacNotch | Gần Figghost nhất. 19 module bật tắt được; hồ sơ dashboard (Quick, Work, Personal, Focus) đổi theo giờ và theo Focus của macOS; Liquid Glass (beta); AI Coding (Claude Code, Cursor Agent) có Allow và Deny; snap cửa sổ; dịch. Không có lịch sử clipboard. | Chính trang so sánh thừa nhận phần Settings "sâu", cần vài phút chọn module. | macnotch.io, cập nhật 9/8/2026 (hãng tự viết) |
| Notchy | Miễn phí, tự liệt kê 51 đến 74 tính năng tùy trang. Bảng lệnh (Ctrl-Option-K); "Knock" (nhấn đúp phím bổ trợ để mở notch); tách bất kỳ tab nào thành popover thanh menu; sáu kiểu kính có tên (Smoke, Onyx, Graphite, Frost, Mist, Crystal) xếp theo mức nhìn xuyên; theo dõi mức dùng AI (Claude Code, Codex, Cursor, Copilot); 134 ngôn ngữ. | Chạy đua số tính năng. | notchy.dev (hãng tự viết) |
| Alcove | Cảm giác Dynamic Island; hoạt hình và cử chỉ được khen; ít module. Trang web dùng nền kem và tiêu đề rất đậm, tức là cá tính rõ dù app là viên thuốc đen. | Bắt chước Dynamic Island (đã là địa hạt của Alcove và DynamicLake). | macnotch.io; ảnh meta của tryalcove.com |
| DynamicLake Pro | Kiểu Liquid Glass, chế độ miniLake gọn, kèm clipboard và chuyển đổi tệp. | Quảng bá quá thiên về mô phỏng Dynamic Island nên chiều sâu tiện ích bị che. | macnotch.io |
| Boring Notch | Mã nguồn mở, khoảng 10.000 sao GitHub. Trình hiển thị âm nhạc theo thời gian thực làm notch "sống". | Visualizer tốn CPU; phần cài đặt đơn giản hơn bản trả phí. | macnotch.io; getseam.app |
| Atoll | Mã nguồn mở. Số liệu hệ thống, clipboard, widget màn hình khóa. | Chỉ chạy trên Mac có notch thật. | macnotch.io |
| Seam | Đi ngược số đông: gần như không cần cấu hình, chỉ cập nhật khi có thay đổi (tiết kiệm pin). Là đối trọng với 17 trang cài đặt của Figghost. | Quá ít tùy chỉnh để làm hộp công cụ. | getseam.app, 6/10/2026 (hãng tự viết) |
| NotchNook | Hạng mục khai sinh của loại app này. Theo Seam (6/10/2026) hiện không bán được. Kiểm tra độc lập ngày 20/10/2026: tên miền `lo.cafe` không phân giải được và trang Setapp của NotchNook chuyển về danh mục chung. Chưa kiểm chứng được phần Homebrew. | Không còn là mốc để so. | getseam.app; kiểm tra bằng curl |
| MediaMate | Thay HUD âm lượng và độ sáng. Liên quan tới bảy kiểu volume indicator của Figghost. | Không. | macnotch.io |

### Nhóm B: sản phẩm làm kỹ ở cài đặt và nhận diện

| Phần mềm | Điều đã xác nhận | Học gì |
|---|---|---|
| Raycast v2 (14/5/2026) | Làm lại từ nền cho macOS Tahoe. Blog nói Liquid Glass được dùng "tasteful ways" để tăng tính chức năng của launcher, và Settings "got a whole revamp". Ảnh Settings: ô tìm kiếm dạng viên ở đầu sidebar, khối tài khoản, mục sidebar là ô icon nhỏ bo góc, nút lùi và tiến lịch sử ở đầu nội dung, trang mở bằng logo lớn kèm một dòng mô tả. Hình quảng bá là các tấm kính 3D trên nền đen. | Nút lùi và tiến; khối đầu trang có hình đại diện; một hình tượng riêng (tấm kính 3D) làm nhận diện. Không sao chép vẻ đen của Raycast. |
| Linear | "Settings are not a design failure" (2/2/2022): làm trang chủ của Settings như một bước onboarding, đặt mẹo và hướng dẫn ngay trong cài đặt; người dùng thích chỉnh để thấy như ở nhà. Bài redesign (28/3/2024): giảm nhiễu thị giác, căn chỉnh nhãn, icon, nút trong sidebar, tăng thứ bậc và mật độ điều hướng; sinh bảng màu từ ba biến (màu nền, màu nhấn, độ tương phản) thay cho 98 biến. | Trang Home của Figghost thành bảng trạng thái và mẹo, không chỉ là nơi có công tắc khởi động. |
| Apple Design Awards 2026, hạng mục Delight and Fun | grug (Ocho) thắng ở mục Apps: giám khảo khen thiết kế "vẽ nguệch ngoạc" nổi bật, thanh trạng thái vẽ tay và cả ghi chú phát hành. Blippo+ (Panic, macOS) có trong cùng hạng mục, giám khảo khen chăm chút chi tiết "xây dựng thế giới": phông, nút, chữ số pixel khổng lồ. | Cá tính nằm ở chi tiết nhỏ (ghi chú phát hành, thanh trạng thái, trạng thái trống) và một nhân vật hợp với chức năng. Figghost đã có nhân vật. |

## 4. Khoảng trống để tạo khác biệt

1. Trùng tính năng với đối thủ ở ba mảng: tab trong notch, snap cửa sổ, Allow và Deny cho AI agent (MacNotch có cả ba). Notchy miễn phí tự liệt kê 51 tính năng ở trang kính của họ và 74 ở trang chủ (bản 1.0.178). Chạy đua số tính năng không còn là lợi thế; sự mạch lạc và cá tính của giao diện mới là chỗ thắng được.
2. Trong ba trang so sánh đã đọc (macnotch.io, getseam.app, notchy.dev) không có app nào nêu một nhân vật làm trục nhận diện. Figghost đã có con ma, sáu nhân vật pixel, màn hello và linh vật clipboard nhưng chỉ dùng ở vài chỗ.
3. Seam cho thấy có người muốn ít cấu hình. Figghost nên giữ chiều sâu nhưng ẩn bớt độ phức tạp (mặc định tốt, mục nâng cao gấp lại).

## 5. Đề xuất

Đây là hướng để chọn và xem thử, chưa phải thiết kế chốt. Các con số (bán kính, số nhóm, tên nhóm) là điểm xuất phát.

### 5.1 Ngôn ngữ thị giác: kính là cửa sổ, nội dung là tờ giấy

Trạng thái (20/10/2026): hai ý đầu (kính thật ở sidebar, thẻ đặc) chưa làm, vì 1.4.0 chọn thẻ kính có chủ đích và canvas đã bán trong suốt nên lớp kính của cửa sổ lộ ra; ý bầu không khí đã làm bằng lớp "Ghost world": trời theo giờ (bình minh, ngày, hoàng hôn, đêm có sao ở chế độ tối), thay cho ánh sáng tím xanh cố định, và chữ viết tay cho tên và lời con ma.

- Cho phần kính thật của cửa sổ lộ ra ở cột sidebar: bỏ nền đặc của `#app` ở cột đó và bỏ `backdrop-filter` của `#sidebar`, chỉ tô canvas đặc cho cột nội dung. HIG Materials nói biến thể Liquid Glass đổi theo lựa chọn giao diện Liquid Glass của người dùng và theo cài đặt giảm trong suốt, nên sidebar kiểu này có khả năng theo thanh trượt của macOS 27 mà không cần mã riêng, và có thể kéo sát mép cửa sổ như macOS 27. Chưa thử với `NSGlassEffectView` của Figghost. Dự kiến chủ yếu là CSS, nhưng cần thử trên cửa sổ thật vì nền trong suốt trong WKWebView có thể có ngoại lệ.
- Thẻ nội dung là bề mặt đặc ba mức (nền, thẻ, thẻ nổi) theo HIG. Bỏ `backdrop-filter` trên `.card`. Bán kính 30 px làm thẻ giống viên thuốc; thử 18 đến 22 px.
- Bầu không khí lấy từ ảnh hero: đen với ánh sáng tím xanh ở chế độ tối; chế độ sáng giữ lavender nhạt nhưng thẻ trắng đặc, đủ tương phản. Nhờ đó cài đặt và notch trông như một sản phẩm.
- Màu: giữ `--tint` theo hệ thống cho điều khiển; `--good`, `--warn`, `--bad` cho trạng thái. Nếu thêm màu theo nhóm thì tránh xanh lá, cam, đỏ (đã dành cho trạng thái theo quy ước hiện tại); dùng xanh dương, tím, hồng, xám xanh.

### 5.2 Điều hướng (sidebar)

Trạng thái (20/10/2026): ý 1 (nhóm có tên) đã có ở 1.4.2 (năm nhóm và tab con). Màu icon: 1.4.0 chọn ô icon màu theo nhóm; HIG cho phép màu cố định khi có mục đích, và nhóm là một mục đích. Chưa làm: chấm trạng thái, khối cần chú ý (con ma ở chân sidebar nay báo quyền Trợ năng bằng thức hoặc ngủ), bảng lệnh Cmd+K dạng lớp phủ (phím Cmd+K đã đưa con trỏ vào ô tìm như Cmd+F), nhớ trang giữa các lần mở, ẩn trang Cài đặt của tính năng đang tắt. Làm khác ý 6: tab của notch thuộc tính năng đang tắt không còn nằm trong thanh tab (xem `Notch::set_page_dormant`), nên notch mặc định có bảy tab như bản 1.4.2 và tab hiện ra khi bật tính năng.

1. Nhãn nhóm, thu gọn được và nhớ trạng thái. Gợi ý theo `groupOf` hiện có: Trang chủ (General, Shortcuts); Hằng ngày (Scrolling, Clipboard, Always on top, Cursor, Window snapping); Notch (Notch, AI agents, Calendar); Công cụ (Smart Select, Translate, Capture, Colour picker); Thêm (Drag wheel, Mole, About). Tiếng Anh: Home, Everyday, Notch, Tools, More.
2. Chấm trạng thái ở mỗi mục: bật, tắt, cần quyền. Dùng đúng `--good`, `--warn`.
3. Khối "cần chú ý" (thiếu Accessibility, có bản cập nhật) ở đầu sidebar hoặc đầu Home, thay cho viên thuốc ở chân.
4. Bảng lệnh Cmd+K. Đã có sẵn chỉ mục chữ của từng hàng (`pageTexts`) và tô sáng (`reveal`); phần mới là lớp phủ giữa cửa sổ và thêm vài hành động (kiểm tra cập nhật, mở notch, mở thư mục cài đặt).
5. Mở lại trang xem gần nhất (HIG Settings); cần một khóa trong `config.json`. Đặt tiêu đề cửa sổ theo trang.
6. Tùy chọn: ẩn trang của tính năng đang tắt (HIG nói nên cho tùy biến sidebar).

Về màu icon: nên giữ màu nhấn hệ thống làm mặc định (HIG, 8/6/2026). Nếu muốn nhận diện nhóm, đặt màu nhóm ở ô icon của tiêu đề trang chứ không ở từng icon sidebar. Đây là chỗ cần chọn.

### 5.3 Các trang: bốn khuôn thay cho một

Trạng thái (20/10/2026): chưa làm bốn khuôn. Đã làm dải số liệu ở trang General (tính năng đang bật, tab notch, mục đã chép, quyền Trợ năng) và trạng thái trống có con ma (`.empty`).

- Khuôn tính năng (Scrolling, Clipboard, Calendar, AI agents): tiêu đề chuẩn rồi các thẻ hàng.
- Khuôn thư viện (Cursor, kích thước Notch, Mascot): ô lớn để chọn, có xem trước.
- Khuôn công cụ (Mole, Capture, Smart Select): hành động trước, kết quả ở giữa, trạng thái trống có nhân vật.
- Khuôn tham chiếu (Shortcuts): bảng phím.

Tiêu đề trang chuẩn: ô icon (màu nhóm), tên, chip trạng thái, công tắc chính, một dòng mô tả. Bỏ các thẻ "công tắc chính" riêng ở đầu mỗi trang ("Notch panel", "Window snapping", "Follow coding agents") vì tiêu đề đã làm việc đó.

Dải xem trước ở đầu năm trang ưu tiên: Notch (đã có), Clipboard, AI agents, Calendar, Mole. Có thể dùng lại bố cục của ảnh render notch.

Home (General): bảng trạng thái (quyền, tính năng bật tắt, phiên bản, có gì mới) và một mẹo luân phiên, theo cách Linear dùng Settings để dạy sản phẩm.

### 5.4 Notch

Trạng thái (20/10/2026): chưa làm các ý về tab và kiểu kính. Các đảo mới của lô 1 (sạc và pin, Caps Lock, ổ đĩa, tải xong, camera và micro, giữ thức, micro, khóa phím) dùng chỗ `notch.set_activity` sẵn có.

- Thanh tab hiện chỉ có icon: thêm nhãn cho tab đang chọn hoặc khi rê chuột.
- Đặt tên cho các kiểu notch (đã có glass, clear, black trong `notch_shots`), có ô xem trước trong trang Notch như phần kích thước. Notchy đặt tên theo mức nhìn xuyên.
- Quầng sáng thương hiệu quanh viên thuốc khi có hoạt động sống (agent đang chờ trả lời), giống ảnh hero.
- Nhân vật ở trạng thái trống của các tab.

### 5.5 Cá tính

Trạng thái (20/10/2026): đã làm: con ma ở chân sidebar (thức hoặc ngủ), con ma hướng dẫn ở mỗi trang với bong bóng và cú nhảy khi bấm, trạng thái trống có nhân vật. Chưa làm: ghi chú phát hành theo giọng con ma, hình khối kính cho mỗi nhóm trang.

- Con ma ở chân sidebar thay cho viên thuốc "Accessibility on": tỉnh khi đủ quyền, ngủ hoặc nhìn quanh khi thiếu; bấm vào đi thẳng tới bước cấp quyền.
- Trạng thái trống có nhân vật (Mole, clipboard rỗng, Agents chưa kết nối).
- Ghi chú phát hành và popup What's new theo giọng của con ma (grug được Apple khen riêng ở ghi chú phát hành).
- Một hình khối kính cho mỗi nhóm trang, cùng kỹ thuật với ảnh hero (agy).
- Không nên: tô cầu vồng cho icon sidebar, thêm kính vào lớp nội dung, bắt chước Dynamic Island hay vẻ đen của Raycast, chạy đua số tính năng.

## 6. Thứ tự làm (ước lượng thô, chưa đọc hết các trang plugin)

1. Điều hướng: nhãn nhóm, thu gọn, chấm trạng thái, khối cần chú ý, nhớ trang. Nhỏ, rủi ro thấp. Chạm `renderNavItems` và `buildShell` trong `app.js`, thêm chuỗi cho cả hai ngôn ngữ, cập nhật fixture và các `expect` của `selftest` nói về `.nav-item`.
2. Lớp nội dung: kính thật ở sidebar, thẻ đặc, tiêu đề chuẩn, Home. Vừa. Chạm khối "Liquid glass theme" của `app.css` và vài trang plugin.
3. Cmd+K và dải xem trước. Vừa.
4. Nhân vật, trạng thái trống, quầng sáng notch, hình hero theo nhóm. Phụ thuộc tài nguyên hình.

Kiểm chứng sau mỗi bước: `settings_preview selftest`, `settings_preview snap <trang> <ảnh> [--dark]`, `render_notch`, `tray_preview`; cập nhật ảnh trong README và đoạn Design của README.

## 7. Chưa chắc chắn

1. Lớp kính thật của cửa sổ có bị `#app` che hoàn toàn hay không: suy ra từ CSS, cần thử trên cửa sổ thật ở macOS 27 với thanh trượt ở cả hai đầu.
2. Cấu trúc cửa sổ cài đặt của MacNotch, NotchNook, Alcove, CleanShot X, iStat Menus, Rectangle Pro, Bartender: chưa có ảnh hay nguồn đáng tin, nên không dùng làm căn cứ.
3. Mọi nhận xét về tính năng và giá của app nhóm A lấy từ trang của chính hãng hoặc của đối thủ; chưa cài đặt hay dùng thử.
4. Ảnh Settings của Raycast chỉ là một góc cắt từ trang quảng bá.
5. Số ngày công ở mục 6 là ước lượng, chưa dựa trên việc đọc từng trang plugin.

## Nguồn

- Apple HIG Sidebars: https://developer.apple.com/design/human-interface-guidelines/sidebars (thay đổi ngày 8/6/2026)
- Apple HIG Materials: https://developer.apple.com/design/human-interface-guidelines/materials
- Apple HIG Settings: https://developer.apple.com/design/human-interface-guidelines/settings
- MacRumors, 8/6/2026: https://www.macrumors.com/2026/06/08/apple-announces-liquid-glass-improvements/
- 9to5Mac, 22/9/2026: https://9to5mac.com/2026/09/22/macos-27-gives-you-more-control-over-liquid-glass/
- Raycast, The New Raycast, 14/5/2026: https://www.raycast.com/blog/the-new-raycast ; ảnh Settings: https://www.raycast.com/new
- Linear, Settings are not a design failure: https://linear.app/now/settings-are-not-a-design-failure
- Linear, How we redesigned the Linear UI (part II): https://linear.app/now/how-we-redesigned-the-linear-ui
- Apple Design Awards 2026: https://developer.apple.com/design/awards/
- MacNotch, so sánh các app notch: https://macnotch.io/compare/best-mac-notch-apps
- Seam, NotchNook với Boring Notch: https://getseam.app/blog/notchnook-vs-boring-notch-vs-seam
- Notchy, Liquid Glass trong notch: https://notchy.dev/mac-liquid-glass/
- Alcove: https://tryalcove.com/
