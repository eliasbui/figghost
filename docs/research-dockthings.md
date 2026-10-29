# DockThings và Launcher của Figghost

Nghiên cứu DockThings (Synium Software) để chọn những gì đưa vào Figghost, đối chiếu với mã ở bản 2.2.5. Ngày 29/10/2026.

## Nguồn

Trang giới thiệu và trang lịch sử phiên bản của Synium, trang FAQ, trang Mac App Store (mô tả, lịch sử phiên bản, đánh giá của người dùng, nhãn quyền riêng tư), bài giới thiệu của appgefahren.de. Chưa dùng thử: app chỉ bán trên Mac App Store và máy này không cài, nên không có gì đọc từ tệp app. Mọi mô tả dưới đây là lời của Synium hoặc người dùng; chỗ nào là suy ra thì ghi rõ. Nguyên tắc của `feature-gap.md` vẫn giữ: viết lại độc lập từ mô tả công khai, không lấy mã, hình hay tên sản phẩm.

## DockThings là gì

Một app đặt các "dock" nổi ở cạnh màn hình, mỗi dock là lưới ô (tile), mỗi ô giữ một tệp, thư mục, app, trang web hoặc Shortcut và mở nó bằng một cú nhấp. Synium nói đây là cách họ làm lại DragThing (launcher nổi tiếng của Mac thập niên 1990 và 2000, ngừng phát triển vì viết bằng Carbon 32 bit) cho năm 2025. Bản 1.0 ra ngày 03/12/2025, ngay sau khi macOS 26 bỏ Launchpad; bản mới nhất là 1.2 (03/08/2026).

| Mục | Chi tiết |
|---|---|
| Hãng | Synium Software GmbH (Mainz, Đức) |
| Giá | 19,99 USD, đang bán 9,99 USD; không có mua trong app |
| Phân phối | Chỉ Mac App Store (nên chạy trong sandbox) |
| Yêu cầu | macOS 15 trở lên (Synium ghi cả macOS 27), Apple silicon và Intel |
| Dung lượng | Khoảng 4 MB |
| Ngôn ngữ | Anh, Đức, Tây Ban Nha, Pháp, Nga, Bồ Đào Nha |
| Quyền riêng tư | Có thu dữ liệu sử dụng (tương tác sản phẩm) để phân tích, không gắn với danh tính |
| Đánh giá | Chưa đủ lượt để hiện điểm; bốn bài đánh giá từ trung lập đến khen |

## DockThings có gì

| Nhóm | Tính năng |
|---|---|
| Dock | Tạo bao nhiêu dock cũng được, gán từng dock cho một màn hình, gắn vào cạnh màn hình nào cũng được. Tự ẩn khi chuột ở xa, khoảng cách và độ mờ chỉnh được (1.1). Phím tắt toàn cục cho từng dock (sửa lỗi ở 1.0.4) |
| Tab | Mỗi dock có nhiều tab. Đổi tab bằng con lăn hoặc vuốt trackpad, đổi tab khi đang kéo tệp ngang qua dock, sắp lại thứ tự tab trong cài đặt, chuyển tab sang dock khác hoặc tách tab thành dock mới (1.2). Khóa và mở khóa mọi ô trong tab một lần |
| Ô | Tệp, thư mục, app, trang web (cả HTTP), Shortcuts (1.2). Đặt tên, màu, ghim cố định, khóa. Biểu tượng riêng: SF Symbol, emoji hoặc ảnh, có màu tô (1.1). Biểu tượng đen trắng tự đổi theo nền dock (1.2). Thêm nhanh một hàng hoặc một cột ô từ menu chuột phải |
| Thư mục | Menu nội dung thư mục có ảnh thu nhỏ từ menu chuột phải, hoặc mở menu ngay khi nhấp thay vì mở Finder (chọn được) (1.1). Thả tệp lên ô thư mục để chuyển tệp vào đó (sửa lỗi ở 1.0.6) |
| Tệp | Quick Look xem trước không cần mở, menu "Mở bằng" có biểu tượng app, tùy chọn ẩn hiện phần đuôi tên tệp (1.2) |
| Trang web | Chế độ mở mặc định cho ô web mới chọn được (1.1). "Mở trang web mà không cần mở trình duyệt trước" cùng nhãn tuổi 16+ vì "truy cập web không giới hạn" gợi ý có cửa sổ web riêng trong app; đây là suy ra, chưa xác nhận |
| Bàn phím | Điều hướng toàn bộ bằng phím trong dock đang có focus, kể cả đổi tab (1.1) |
| Giao diện | Nhiều mẫu bố cục và kiểu: kính, trong mờ, tương phản cao, "Flat & Square" đục hoặc trong (1.2). Cỡ và độ đậm chữ của tab và của ô chỉnh được |
| Khác | Khởi động cùng máy (1.1), tooltip cho tên bị cắt, URL scheme (1.0.4; ghi chú không nói là scheme của DockThings hay ô giữ được địa chỉ có scheme khác) |

Người dùng phàn nàn ba điều: biểu tượng xử lý kém ở chế độ tối, không đặt được hai dock trên cùng một màn hình, dock quên trạng thái khi đổi máy qua bộ chuyển KVM (màn hình ngắt rồi nối lại). Điều được khen nhất là tab và vuốt trackpad để đổi tab.

## Figghost đã có gì trong số đó

| Tính năng DockThings | Figghost 2.2.5 |
|---|---|
| Ô app, thư mục, Shortcut | Có: tab Launcher trong notch, tắt theo mặc định (`crates/app-ui/src/features/launcher/`) |
| Số ô | Tối đa 12, lưới 6 cột (`MAX_TILES`, `COLUMNS` ở `crates/macos-sys/src/launcher_ui.rs`) |
| Ô tệp, ô trang web | Chưa: `Kind` chỉ có `App`, `Folder`, `Shortcut` (`launcher/logic.rs`) |
| Giữ tệp để dùng lại | Một phần: shelf giữ tệp thả lên notch theo tham chiếu, nhớ qua các lần chạy (`crates/app-ui/src/shelf.rs`), nhưng là chỗ tạm chứ không phải ô ghim có tên |
| Tab, nhóm ô | Chưa |
| Dock nổi ở cạnh màn hình, tự ẩn | Chưa: Launcher chỉ sống trong notch |
| Phím tắt mở | Một phần: `notch.shortcut` mở notch, `notch.open_page` chọn được tab mở ra |
| Menu nội dung thư mục | Chưa: ô thư mục mở Finder |
| Thả tệp lên ô (mở bằng app, chuyển vào thư mục) | Chưa |
| Quick Look | Chưa: không có chỗ nào dùng `QLPreviewPanel` |
| Biểu tượng, màu riêng cho ô | Chưa: ô dùng biểu tượng của hệ thống |
| Điều hướng bằng phím trong lưới | Cần kiểm lại trên tab Launcher |
| Nhiều màn hình | Một phần: notch chọn được màn hình (`notch.display`), snap có danh sách riêng theo tên màn hình (`snap.screens`) |
| URL scheme | Chưa (xem `research-raycast-2.5.0.md`) |
| Nhiều mẫu giao diện | Không theo: Figghost có một ngôn ngữ thiết kế (`docs/design.md`) |

Kết luận: Figghost đã có phần lõi (ô app, thư mục, Shortcut, chạy không qua shell). Cái DockThings làm hơn nằm ở ba chỗ: ô giữ được mọi thứ (tệp, web), ô làm được nhiều việc hơn mở (menu thư mục, thả tệp, Quick Look), và nhiều ô hơn nhờ nhóm. Dock nổi ở cạnh màn hình là phần lớn nhất và là câu hỏi về bản sắc, không phải về tính năng.

## Đề xuất

Làm theo thứ tự, mỗi bước là một bản nhỏ phát hành riêng. Bước 1 và 2 nằm trong tính năng Launcher đã có, nên vẫn tắt theo mặc định và dùng chung trang cài đặt Launcher.

### Bước 1: ô làm được nhiều hơn

1. Thêm `Kind::File` (đường dẫn đầy đủ, mở bằng app mặc định qua `NSWorkspace`) và `Kind::Url`. Ô web chỉ nhận `http`, `https` và `mailto`: cấu hình là ranh giới tin cậy, một scheme lạ trong tệp có thể chạy app khác. Mở bằng trình duyệt mặc định; không làm cửa sổ web riêng.
2. Ô thư mục: chuột phải mở menu nội dung (một cấp, biểu tượng từ `NSWorkspace.iconForFile`). Giới hạn số mục trong menu và ghi `ponytail:` về trần đó.
3. Thả tệp lên ô: ô app mở tệp bằng app đó; ô thư mục chuyển tệp theo đúng quy tắc của Finder (cùng ổ thì di chuyển, khác ổ thì sao chép, giữ Option để sao chép). Không bao giờ ghi đè: trùng tên thì thêm số như Finder, để không mất dữ liệu.
4. Quick Look: phím Space trên ô tệp đang chọn mở `QLPreviewPanel`. Shelf dùng lại được cùng đoạn mã.

### Bước 2: nhóm

Tab Launcher có nhiều nhóm, mỗi nhóm tối đa 12 ô như lưới hiện tại, nên `launcher_ui.rs` giữ nguyên kích thước trang. Một dải tên nhóm ở trên, đổi nhóm bằng vuốt hai ngón hoặc con lăn (điều người dùng DockThings khen nhiều nhất), và đổi nhóm khi đang kéo tệp ngang qua. Tệp cấu hình cũ có `launcher.items` được đọc thành nhóm đầu tiên rồi ghi lại theo dạng mới, như cách `weather.place` thành `weather.places`.

### Bước 3: dock ở cạnh màn hình (cần quyết định)

Một tính năng riêng, tắt theo mặc định, có trang cài đặt riêng: một panel Liquid Glass không lấy focus gắn vào cạnh trái, phải hoặc dưới của màn hình đã chọn, tự ẩn khi chuột ở xa, hiện một nhóm của Launcher. Dùng chung danh sách nhóm và ô của Launcher, không có danh sách thứ hai. Hai bài học từ lời phàn nàn của người dùng DockThings: nhớ vị trí theo tên màn hình như `snap.screens` và đặt lại đúng chỗ khi màn hình nối lại; cho nhiều dock trên một màn hình nếu ở các cạnh khác nhau. Tắt tính năng thì gỡ panel, không để lại gì ngoài app.

## Không làm

| Tính năng | Lý do |
|---|---|
| Nhiều mẫu giao diện và kiểu dock | Figghost giữ một ngôn ngữ thiết kế; panel nổi là Liquid Glass (`docs/design.md`) |
| Cửa sổ web trong app | Thêm một trình duyệt thu nhỏ để giữ và vá; trình duyệt mặc định đã làm tốt |
| Thu dữ liệu sử dụng | Figghost không gửi gì đi khi người dùng không yêu cầu |
| Ảnh riêng làm biểu tượng ô | Để sau: SF Symbol và màu đủ cho nhu cầu phân biệt; thêm khi có người xin |
| Khóa ô | Để sau: Launcher chỉ sửa được từ trang cài đặt, nên không có thao tác kéo nhầm cần khóa |

## Câu hỏi mở cho người quyết định

1. Có làm dock ở cạnh màn hình (bước 3) không, hay giữ mọi thứ trong notch để đúng bản sắc Figghost?
2. Thả tệp lên ô thư mục: theo quy tắc di chuyển của Finder, hay luôn sao chép cho an toàn?
3. Giới hạn bao nhiêu nhóm trong Launcher (gợi ý 8, như số thành phố của Weather)?
4. Ô web mở bằng trình duyệt mặc định, hay cho chọn trình duyệt cho từng ô?

## Trạng thái (29/10/2026)

Đã làm, chưa phát hành:

1. Bước 1, một phần: ô tệp và ô trang web (`http`, `https`, `mailto`; scheme khác bị từ chối), menu chuột phải cho ô thư mục (tối đa 40 mục, thư mục trước) và cho ô ứng dụng, ô tệp (Mở, Hiện trong Finder).
2. Bước 2: tối đa tám nhóm, mỗi nhóm mười hai ô; dải nhóm trên tab, đổi nhóm bằng vuốt hai ngón, con lăn, phím mũi tên. Tệp cấu hình cũ có `launcher.items` được đọc thành nhóm đầu tiên.
3. Bước 3: dock Liquid Glass ở cạnh trái, phải hoặc dưới, chọn màn hình theo tên, quay về chỗ cũ khi màn hình nối lại, tự ẩn tùy chọn. Có công tắc riêng, tắt theo mặc định.

Chưa làm:

- Thả tệp lên ô (mở bằng app, chuyển vào thư mục): khi kéo tệp, notch tự chuyển sang tab shelf, nên phải sửa cách notch nhận tệp trước.
- Quick Look: `QLPreviewPanel` cần một responder trong chuỗi responder của panel và framework Quartz, chưa có sẵn trong mã.
- Biểu tượng và màu riêng cho từng ô, đổi tên ô.

Các câu hỏi mở ở trên được chốt theo mặc định: có làm dock; ô web mở bằng trình duyệt mặc định; giới hạn tám nhóm. Câu hỏi thả tệp lên thư mục để lại vì chưa làm phần thả tệp.
