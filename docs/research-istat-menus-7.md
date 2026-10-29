# iStat Menus 7 và phần Hệ thống của Figghost

Nghiên cứu iStat Menus 7 (từ bản 7.0 ngày 31/07/2024 tới bản 7.5 ngày 14/09/2026) để đưa cách trình bày của nó vào tab System của notch và các số liệu trên thanh menu. Bản vẽ đề xuất đi kèm: [redesign-system-istat.html](redesign-system-istat.html).

## Nguồn

Trang sản phẩm iStat Menus của Bjango, trợ giúp iStat Menus 7 (Welcome, Global, Menus, Combined), lịch sử phiên bản trên trang Bjango, bài của MacRumors ngày 31/07/2024 và TidBITS về bản 7.0.2, đọc ngày 29/10/2026. Mô tả lấy từ lời của Bjango và báo chí, chưa dùng thử. Ghi chú phát hành 7.5 mới chỉ có "More details coming soon"; trang sản phẩm nói 7.5 có menu Liquid Glass cho macOS 27 và một mục Astronomy mới.

Nguyên tắc của `feature-gap.md` vẫn giữ: viết lại độc lập từ mô tả công khai, không lấy mã, hình, theme hay tên sản phẩm.

## iStat Menus 7 có gì

| Nhóm | Tính năng |
|---|---|
| Mục trên thanh menu | Chín mục riêng (CPU và GPU, bộ nhớ, ổ đĩa, mạng, cảm biến, quạt, nguồn, thời gian, thời tiết), bật tắt và đổi thứ tự (Command-kéo). Mỗi mục ghép từ các phần tử hiện hoặc ẩn được: bánh tròn, đồ thị, đồ thị lịch sử, phần trăm, nhãn, biểu tượng |
| Kiểu hiển thị mới ở 7.0 | Hai dòng: nhãn trên giá trị, hoặc nhãn trên thanh, cho CPU, GPU, bộ nhớ, ổ đĩa, cảm biến, nguồn. Kiểu biểu tượng cho CPU, bộ nhớ, ổ đĩa; kiểu ping cho mạng; đồ thị pin kiểu iOS. 7.3 thêm kiểu biểu tượng giao diện mạng và nhóm lại nhân CPU trên thanh menu |
| Gộp (Combined) | Một mục duy nhất chứa nhiều số liệu; chọn riêng thứ hiện trên thanh và các khối trong menu. Bjango nói nó hợp nhất với máy xách tay |
| Menu thả xuống | Mỗi mục một menu: đồ thị lịch sử, các khối chi tiết, danh sách app dùng nhiều nhất. Menu con khi rê chuột: chi tiết thành phố, mặt trời và mặt trăng, tốc độ liên kết mạng. Tự chỉnh menu ngay trên bản xem trước trong Cài đặt: rê chuột lên thì hiện công tắc và tay kéo đổi thứ tự |
| CPU và GPU | Tải từng nhân, nhân tiết kiệm và nhân hiệu năng tô khác nhau, lịch sử, tải trung bình, thời gian chạy, tần số trên Apple silicon, app dùng nhiều CPU nhất. GPU: tải, bộ nhớ, nhiệt, tần số, số khung hình mỗi giây |
| Bộ nhớ | Đang dùng, lịch sử, áp lực, bộ nhớ nén, swap, app dùng nhiều nhất |
| Ổ đĩa | Đã dùng, còn trống, đọc ghi, S.M.A.R.T., dùng ổ theo app |
| Mạng | Tải lên và tải về với đồ thị lịch sử, băng thông theo app, chi tiết kết nối, IP công cộng và nội bộ, đèn báo có Internet, tên Wi-Fi, tốc độ liên kết, vị trí mạng |
| Cảm biến và quạt | Nhiệt, quạt, điện áp, dòng, công suất; điều khiển quạt theo nhiệt hay theo pin, tăng dần |
| Nguồn | Mục thanh menu đổi theo đang xả, đang sạc, đã đầy; pin AirPods, Magic Mouse, Magic Trackpad; đồ thị lịch sử |
| Thời gian và thời tiết | Đồng hồ thanh menu theo định dạng tự đặt, lịch 7 hay 14 ngày cuộn, đồng hồ thế giới có mặt trời và mặt trăng, cờ quốc gia; thời tiết theo giờ và theo tuần |
| Quy tắc | Thông báo khi IP công cộng đổi, mất Internet, CPU cao kéo dài, ổ sắp đầy, và theo mọi nhóm số liệu |
| Toàn cục | Theme riêng cho thanh menu và cho menu, chọn trên một băng cuộn; tần suất đọc; khoảng cách giữa các mục chuẩn hay gọn |

## Vì sao trông đẹp

1. **Thanh menu đơn sắc.** Mặc định mọi mục vẽ bằng màu chữ của thanh menu, nên đứng cạnh biểu tượng của hệ thống không lạc tông. Màu chỉ có ở đồ thị khi người dùng chọn.
2. **Hai dòng trong 22 điểm.** Nhãn rất nhỏ ở trên, giá trị ở dưới: `CPU` trên `12%`. Một số liệu chỉ tốn khoảng 38 điểm chiều ngang thay vì 60 đến 90 của một dòng chữ, đó là cách nó nhét năm sáu số liệu vào một thanh menu bị notch cắt.
3. **Đồ thị tí hon có nghĩa.** Cột từng nhân chia nhóm tiết kiệm và hiệu năng, đồ thị lịch sử một phút, bánh tròn bộ nhớ. Nhìn hình là biết máy đang bận hay rảnh, không cần đọc số.
4. **Bề rộng cố định.** Số thay đổi mỗi giây nhưng các mục không xô nhau.
5. **Menu có thứ bậc rõ.** Số lớn ở đầu, đồ thị ngay dưới, rồi các khối có tiêu đề chữ hoa nhỏ, số căn phải bằng chữ số đều, cuối là danh sách app. Mắt đi một đường từ trên xuống.
6. **Màu tiết chế.** Bản 7.0 bỏ các tông trung tính để dùng hai màu chính là đỏ và xanh nhạt; theme mặc định lấy màu nhấn của macOS. Hai chuỗi số liệu trong một đồ thị (tải lên, tải về; user, system) chỉ cần hai màu đó.
7. **Đồ thị đối xứng cho mạng.** Tải về phía trên trục, tải lên phía dưới, cùng một khung, đọc được cả hai mà không chồng lên nhau.
8. **Nhẹ.** Bjango nói bản 7.0 dùng khoảng 20% lượng CPU của bản 6.

## Figghost đã có gì trong số đó

| Tính năng iStat Menus | Figghost 2.2.5 |
|---|---|
| Tải từng nhân, nhóm tiết kiệm và hiệu năng | Có, tab System |
| Tải trung bình, thời gian chạy, tần số | Chưa |
| App dùng nhiều CPU và bộ nhớ nhất | Chưa ở tab System; `sysinfo::processes()` đã đọc (Mole dùng), chỉ tiến trình của người dùng, không cần quyền |
| Bộ nhớ, áp lực, nén, swap | Có |
| GPU | Tải (`sysinfo::gpu_percent`); chưa có khung hình mỗi giây của hệ thống |
| Ổ đĩa | Còn trống của ổ khởi động; chưa có đọc ghi, S.M.A.R.T. |
| Mạng | Tốc độ, một phút lịch sử; chưa có IP, tên Wi-Fi, ping, băng thông theo app |
| Nhiệt, quạt | Có đọc (`sensors`); không điều khiển quạt |
| Pin | Sức khỏe pin trong tab, sạc và pin yếu ở Đảo trạng thái, pin thiết bị Bluetooth ở trang Thiết bị |
| Thời tiết, lịch | Có, tab Thời tiết và tab Lịch |
| Đồng hồ thế giới | Một phần: Chọn thông minh đổi giờ của chữ được chọn |
| Quy tắc thông báo | Chưa; Đảo trạng thái đã có chỗ hiện |
| Mục thanh menu | Một mục chữ thường `CPU 12%  RAM 54%  NET 1.2 MB/s`, chữ số đều, bấm thì mở tab notch. Chưa có kiểu hai dòng, đồ thị, mục tách riêng |
| Menu thả xuống | Chưa: bấm mở tab notch |

Kết luận: số liệu Figghost đã đọc gần đủ. Thứ thiếu là **cách trình bày**: thanh menu chỉ là một dòng chữ, không có menu thả xuống dưới số liệu, và tab System dùng năm màu (xanh, cam, tím, lục, đỏ) cho các phần khác nhau. Đó đúng là chỗ iStat Menus 7 đẹp hơn.

## Đề xuất

Ba bước, mỗi bước phát hành được riêng. Bước 1 đáng làm nhất.

### Bước 1: số liệu trên thanh menu vẽ như iStat (đã làm)

- Mỗi số liệu là một **ô** vẽ thành ảnh template, nên tự theo màu thanh menu (sáng, tối, nền tô màu, Tăng độ tương phản). Bề rộng mỗi ô cố định theo kiểu của nó.
- Mỗi ô chọn một trong bốn **kiểu**: Chữ (`12%`), Hai dòng (nhãn 7 điểm trên, giá trị 10 điểm dưới), Đồ thị (30 lần đọc gần nhất, rộng 28 điểm), Cột nhân (một vạch 2 điểm mỗi nhân, khe giữa nhóm tiết kiệm và nhóm hiệu năng). Mạng có kiểu riêng: hai dòng tải lên và tải về có mũi tên.
- **Ô có sẵn**: CPU, bộ nhớ, mạng như hôm nay; thêm GPU, nhiệt CPU, ổ khởi động, đều đọc bằng hàm đã có.
- **Gộp hay tách**: mặc định gộp thành một mục như hôm nay (ít chỗ, nhất là máy có notch); tùy chọn tách mỗi ô một mục để Command-kéo đổi thứ tự.
- **Màu**: mặc định đơn sắc. Tùy chọn "Đồ thị có màu" tô đồ thị bằng màu nhấn, đỏ khi quá 85%.
- Không đọc thêm gì so với hôm nay ngoài ô được chọn, cùng nhịp `seconds`.

### Bước 2: menu thả xuống Liquid Glass

- Bấm một ô mở một bảng kính ngay dưới nó, đóng khi bấm ra ngoài hoặc Esc. Tùy chọn "Bấm vào số liệu: Mở menu hoặc Mở tab notch" để ai quen notch vẫn giữ.
- Bảng rộng 320 điểm, cùng kính với notch (`notch::glass_tint`), bản mờ đục khi Giảm độ trong suốt. Từ trên xuống: số lớn và trạng thái, đồ thị lịch sử với nút chọn `1 phút`, `1 giờ`, `1 ngày`, các khối có tiêu đề chữ hoa nhỏ và số căn phải, khối "Dùng nhiều nhất" năm app có biểu tượng.
- **CPU**: user và system hai màu, nhân theo nhóm, tải trung bình 1, 5, 15 phút (`getloadavg`), thời gian chạy (`kern.boottime`), năm app theo CPU.
- **Bộ nhớ**: vòng tròn app, wired, nén bằng sắc độ màu nhấn, áp lực, swap, năm app theo bộ nhớ.
- **Mạng**: đồ thị đối xứng, từng giao diện (Wi-Fi, Ethernet) với tổng đã truyền, IP nội bộ. IP công cộng là tùy chọn tắt sẵn vì phải gọi ra ngoài.
- **GPU, nhiệt và quạt, ổ đĩa**: mỗi thứ một bảng ngắn.
- **Lịch sử dài** giữ trong bộ nhớ: trung bình mỗi phút, 1440 điểm cho 24 giờ mỗi chuỗi, vài chục KB, chỉ khi có ô trên thanh menu. Không ghi đĩa; ghi khi có người cần lịch sử qua lần khởi động lại.

### Bước 3: tab System trên notch cùng ngôn ngữ

- Bỏ năm màu. CPU một màu nhấn, nhân tiết kiệm là sắc nhạt của nó, đỏ chỉ khi quá 85%. Bộ nhớ chia sắc độ màu nhấn như biểu đồ của Mole. Mạng dùng đồ thị đối xứng: tải về màu nhấn, tải lên đỏ nhạt.
- Tiêu đề khối chữ hoa nhỏ, mọi số bằng chữ số đều (`style::digits` đã có).
- Bố cục giữ nguyên ba thẻ lớn và năm thẻ nhỏ, để không phải đổi `PAGE_SIZE` của notch.

### Để sau

- Quy tắc thông báo (CPU cao kéo dài, ổ gần đầy, mất Internet) hiện qua Đảo trạng thái, khi có người hỏi.
- Đồng hồ thế giới và Thiên văn: tab Lịch và tab Thời tiết gần với chúng hơn tab System.
- Menu con khi rê chuột lên một app (đường dẫn, PID, nút Thoát).

### Không làm

- **Điều khiển quạt**: cần một helper chạy root ghi vào SMC, có rủi ro cho phần cứng và trái nguyên tắc tab System không cần quyền nào.
- **Băng thông và đọc ghi ổ theo app**: cần framework riêng tư `NetworkStatistics` hoặc chạy `nettop`.
- **S.M.A.R.T.**: ít người xem, máy Apple silicon báo rất ít.
- **Băng cuộn theme nhiều màu**: trái quy tắc một màu của `docs/design.md`. Figghost có màu nhấn của hệ thống và kính, thế là đủ.

## Trang Cài đặt

Trang System (tab notch) thêm hai nhóm, theo `docs/design.md` (một màu, nhóm ít dùng sau Nâng cao…):

1. **Thanh menu**: sáu dòng, mỗi dòng một ô chọn Tắt hoặc một kiểu; Gộp thành một mục; Đồ thị có màu. Bản đã làm bỏ tay kéo và bản xem trước: thứ tự là thứ tự trong danh sách, mục tách riêng thì Command-kéo, và chính thanh menu là bản xem trước. "Bấm vào số liệu" đến cùng bước 2.
2. **Menu** (Nâng cao…): khoảng lịch sử mặc định, số app trong danh sách, IP công cộng.

Mọi số liệu mới tắt sẵn. Kiểu mặc định là Hai dòng, nên người đang bật `bar_processor`, `bar_memory`, `bar_network` thấy ngay kiểu mới, vẫn gộp một mục; chọn Chữ để trở lại như cũ. Tắt tính năng thì mọi mục rời thanh menu.

## Cách làm

| Bước | Chỗ sửa |
|---|---|
| 1 | `macos-sys/src/status_item.rs`: `TextItem` nhận ảnh template. Một tệp mới trong `macos-sys` vẽ các ô thành `NSImage`. `app-ui/src/features/system/logic.rs`: bề rộng và chữ của từng ô, có unit test. Cấu hình mới trong `system/settings.rs` và `system.fixture.json`, chuyển ba khóa `bar_*` cũ |
| 2 | Bảng kính dựng trên `panel::FigPanel` như cửa sổ Dịch (`translate_ui.rs`). Vẽ bằng AppKit như `system_ui.rs`, không dùng web view, để giữ 25 MB. Lịch sử phút trong `logic.rs` |
| 3 | Chỉ `system_ui.rs` (màu và đồ thị mạng) và ảnh `docs/images/notch-system.png` vẽ lại bằng `examples/render_system.rs` |

Kiểm tra: unit test cho bề rộng ô, chữ, chuyển cấu hình và lịch sử phút; `render_system` vẽ thêm dải thanh menu và từng bảng để so với bản vẽ; kính chỉ kiểm được bằng ảnh chụp màn hình thật.
