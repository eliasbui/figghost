<p align="center">
  <img src="docs/images/icon.png" width="112" alt="">
</p>

<h1 align="center">figghost</h1>

<p align="center">
  Hộp công cụ gọn nhẹ, bản địa, cho thanh menu và notch của máy Mac.<br>
  Cuộn chuột mượt, lịch sử clipboard, chia cửa sổ, gõ tiếng Việt, âm thanh riêng cho từng ứng dụng, chụp màn hình và một notch có tab cho mọi công cụ nhỏ.
</p>

<p align="center">
  <a href="README.md"><img src="https://img.shields.io/badge/English-grey?style=for-the-badge" alt="English"></a>
  <a href="README.vi.md"><img src="https://img.shields.io/badge/Ti%E1%BA%BFng%20Vi%E1%BB%87t-blue?style=for-the-badge" alt="Tiếng Việt (hiện tại)"></a>
</p>

<p align="center">
  <a href="https://github.com/eliasbui/figghost/releases/latest"><img src="https://img.shields.io/github/v/release/eliasbui/figghost?label=release" alt="Bản phát hành mới nhất"></a>
  <img src="https://img.shields.io/badge/macOS-13%2B-black" alt="macOS 13 trở lên">
  <img src="https://img.shields.io/badge/Apple%20silicon-arm64-black" alt="Apple silicon">
  <a href="#windows-và-linux-beta"><img src="https://img.shields.io/badge/Windows%20and%20Linux-beta-orange" alt="Windows và Linux: beta"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="Giấy phép MIT"></a>
</p>

<p align="center">
  <img src="docs/images/hero.jpg" width="100%" alt="Một viên notch đen có chú ma, phía trên là các bảng kính mờ: danh sách clipboard, lưới bố cục cửa sổ, thanh âm lượng và ô lịch">
</p>

Figghost viết bằng Rust với lớp vỏ AppKit thuần (các crate `objc2`). Cửa sổ cài đặt là một trang web trên cửa sổ Liquid Glass; mọi quy tắc phía sau đều nằm trong mã Rust có kiểm thử. Ứng dụng chạy trên thanh menu, dùng khoảng 25 MB khi cửa sổ cài đặt đóng, và có tiếng Anh lẫn tiếng Việt.

Đã có **bản beta đầu tiên cho Windows và Linux** gồm biểu tượng ở khay hệ thống, lịch sử clipboard, ghi chú, AI agents và MCP server; notch và các tính năng macOS khác chưa có. Xem [Windows và Linux (beta)](#windows-và-linux-beta).

## Mục lục

- [Danh sách tính năng](#danh-sách-tính-năng)
- [Cửa sổ cài đặt](#cửa-sổ-cài-đặt)
- [Điểm nổi bật](#điểm-nổi-bật)
- [Tính năng chi tiết](#tính-năng-chi-tiết)
- [Cài đặt](#cài-đặt)
- [Build và chạy](#build-và-chạy)
- [Windows và Linux (beta)](#windows-và-linux-beta)
- [Thiết kế](#thiết-kế)
- [Cấu hình](#cấu-hình)
- [Dữ liệu và riêng tư](#dữ-liệu-và-riêng-tư)
- [Lựa chọn công nghệ](#lựa-chọn-công-nghệ)
- [Phát triển](#phát-triển)

## Danh sách tính năng

Cửa sổ cài đặt gom mọi tính năng dưới sáu biểu tượng trên một dock ở chân cửa sổ, và danh sách này đi theo đúng sáu nhóm và tên trang đó: tên trong giao diện tiếng Việt, kèm tên tiếng Anh. Tính năng ghi "tắt mặc định" được bật ở trang riêng của nó.

### Cài đặt chung (General)

| Trang | Tiếng Anh | Chức năng |
| --- | --- | --- |
| Chung | General | Mở khi đăng nhập, lời chào của chú ma, giao diện sáng tối và ngôn ngữ, kiểm tra mọi quyền bằng một lần bấm, và ô bật tắt cho từng tính năng chính |
| Mole | Mole | Điểm sức khỏe máy Mac và dọn bộ nhớ đệm, thư mục build cũ, tệp cài đặt, ứng dụng cùng phần còn sót, trong các danh sách tự hiện ra và xóa nhiều mục cùng lúc (tắt mặc định) |
| Giới thiệu | About | Phiên bản, mỗi ngày tìm bản mới và cài ngay trong ứng dụng, có gì mới, riêng tư, thư mục dữ liệu và giấy phép |

### Chuột & Thao tác (Mouse & Cursor)

| Trang | Tiếng Anh | Chức năng |
| --- | --- | --- |
| Cuộn chuột | Scrolling | Cuộn bánh xe mượt với đường cong nhìn thấy được, đảo chiều chỉ cho chuột ngoài, phím bổ trợ để cuộn ngang, nhanh hơn và phóng to, cùng quy tắc riêng cho từng ứng dụng |
| Con trỏ | Cursor | Mũi tên cầu vồng, nhân vật pixel hoặc ảnh, GIF bất kỳ làm con trỏ (tắt mặc định) |
| Vòng kéo thả | Drag wheel | Một vòng quanh con trỏ khi bạn kéo tệp: đổi cỡ, nén, tách hoặc gộp PDF, chú thích, chép chữ trong ảnh, cắt chủ thể, xóa siêu dữ liệu, đổi video thành GIF |

### Notch & Tiện ích (Notch & Island)

| Trang | Tiếng Anh | Chức năng |
| --- | --- | --- |
| Notch | Notch | Hình khối mọc ra từ mép trên màn hình: kích thước, chất liệu, quy tắc mở, hành vi trên ứng dụng toàn màn hình, các tab hiển thị, chia cửa sổ và chỉ báo âm lượng bảy kiểu |
| Đảo trạng thái | Live islands | Thông báo nhỏ ló ra từ notch đóng: Caps Lock, sạc, pin, ổ đĩa, tải xuống, camera và micro (tắt mặc định) |
| Widget nhạc | Music widget | Trình phát Liquid Glass nổi trên màn hình nền, năm kiểu, có thể đặt trên màn hình khóa (tắt mặc định) |

### Tab của notch (Notch tabs)

Notch luôn có Clipboard, Khay tệp, Media (kèm lời bài hát), Tập trung và Âm thanh; dưới đây là các trang của những tab bạn thêm vào. Tab ghi tắt mặc định chỉ vào dải tab khi được bật.

| Trang | Tiếng Anh | Chức năng |
| --- | --- | --- |
| Lịch | Calendar | Sự kiện và lời nhắc hôm nay, và báo trước cuộc họp với nút vào liên kết Zoom, Meet, Teams hoặc Webex (tắt mặc định) |
| Tiến độ ngày | Day progress | Ngày đã trôi qua bao nhiêu, với các sự kiện hôm nay nằm dọc một đường (tắt mặc định) |
| Tác tử AI | AI agents | Phiên Claude Code, Codex và pi, giới hạn sử dụng, token và chi phí, trả lời Cho phép hoặc Từ chối và lựa chọn ngay trên notch; tài khoản Orca và chuyển tài khoản |
| Ghi chú | Notes | Nơi nháp bằng các tệp văn bản thuần (tắt mặc định) |
| Mở nhanh | Launcher | Lưới tối đa mười hai ứng dụng, thư mục và Phím tắt Shortcuts (tắt mặc định) |
| Thời tiết | Weather | Tối đa tám thành phố: hiện tại, những giờ và ngày sắp tới (tắt mặc định) |
| Yêu cầu duyệt | Pull requests | Các yêu cầu duyệt đang chờ bạn trên GitHub và GitLab (tắt mặc định) |
| Pomodoro | Pomodoro | Phiên làm việc và nghỉ có vòng tròn tiến độ, chuỗi ngày và phím tắt (tắt mặc định) |
| Thời gian dùng | Screen Time | Mỗi ứng dụng ở phía trước bao lâu, hôm nay và bảy ngày qua (tắt mặc định) |
| Hệ thống | System | Bộ xử lý, bộ nhớ, mạng, đồ họa, ổ đĩa, nhiệt, quạt và sức khỏe pin (tắt mặc định) |
| Thiết bị | Devices | Mức pin của các thiết bị Bluetooth đang kết nối, cảnh báo khi sắp hết (tắt mặc định) |
| Màn hình | Displays | Độ sáng và độ tương phản của màn hình ngoài qua DDC/CI, và phím độ sáng cho chúng (tắt mặc định) |

### Bảng tạm & Công cụ (Clipboard & Tools)

| Trang | Tiếng Anh | Chức năng |
| --- | --- | --- |
| Clipboard | Clipboard | Lịch sử văn bản, liên kết, mã, ảnh và tệp với tìm kiếm tức thì, ghim, Copy Stack, bảng, dữ liệu nhạy cảm bị giữ ngoài, và máy chủ MCP cho trợ lý AI (tắt mặc định) |
| Chọn thông minh | Smart Select | Hiểu chữ đang chọn: đơn vị, tiền tệ, giờ ở các múi khác, bản dịch (tắt mặc định) |
| Dịch | Translate | Chọn chữ ở bất kỳ đâu và một cửa sổ kính cạnh con trỏ dịch nó (tắt mặc định) |
| Gõ tiếng Việt | Vietnamese typing | Telex hoặc VNI ở mọi ứng dụng, VI hoặc EN trên notch và ở thanh menu, phím tắt để chuyển, các ứng dụng giữ nguyên (tắt mặc định) |
| Chụp màn hình | Capture | Chụp màn hình, chụp cuộn, quay màn hình ra MP4 hoặc GIF, trình chỉnh sửa và nhận dạng chữ tiếng Việt, tiếng Anh (tắt mặc định) |
| Chọn màu | Colour picker | Kính lúp trên màn hình đóng băng, chép màu dạng HEX, RGB, HSL hoặc HSB (tắt mặc định) |
| Tiện ích | Utilities | Giữ máy thức, tắt tiếng micro cho mọi ứng dụng, khóa bàn phím khi lau, tạm ẩn notch (tắt mặc định) |

### Cửa sổ & Phím tắt (Window & Keys)

| Trang | Tiếng Anh | Chức năng |
| --- | --- | --- |
| Chia cửa sổ | Window snapping | Bố cục theo từng màn hình từ mẫu có sẵn hoặc lưới, mở bằng cách kéo cửa sổ lên mép trên hoặc bằng phím tắt |
| Luôn trên cùng | Always on top | Một phím tắt giữ cửa sổ ở phía trước với viền đỏ (tắt mặc định) |
| Phím tắt | Shortcuts | Mọi phím tắt của cửa sổ clipboard và notch ở một chỗ, phím trùng bị từ chối |

### Còn có sẵn

Âm thanh riêng cho từng ứng dụng (âm lượng tới 400 %, thiết bị phát, equalizer, bass và treble, âm thanh 3D cho tai nghe), linh vật clipboard, ẩn biểu tượng trên thanh menu, kéo cửa sổ vào notch để chia, lời chào của chú ma mỗi lần bạn mở ứng dụng, và cập nhật luôn hỏi trước khi cài.

## Cửa sổ cài đặt

Mở ứng dụng là mở cửa sổ cài đặt; đóng cửa sổ thì ứng dụng về lại thanh menu. Các nhóm trang là sáu biểu tượng của dock kính ở chân cửa sổ: bấm một biểu tượng, hoặc bấm giữ rồi kéo dọc dock, một thấu kính trong suốt sẽ đi theo ngón tay và mở nhóm nằm dưới nó. Ô tìm ở thanh tiêu đề (Command-K) tìm trang theo nội dung cài đặt của nó.

| | |
| --- | --- |
| ![Trang Chung: phần đầu, các con số và ô bật tắt các tính năng chính](docs/images/settings-general.png) | ![Trang Cuộn chuột của nhóm Chuột & Thao tác](docs/images/settings-scroll.png) |
| ![Trang Gõ tiếng Việt: ngôn ngữ, phím tắt, công tắc notch và thanh menu](docs/images/settings-typing.png) | ![Trang Giới thiệu với phiên bản, cập nhật và danh sách có gì ở đâu](docs/images/settings-about.png) |

## Điểm nổi bật

### Notch, với một tab cho mỗi công cụ

Một hình khối đen mọc ra từ mép trên màn hình khi con trỏ chạm tới: clipboard, khay tệp, media, tác tử AI, lịch, ghi chú, mở nhanh, thời tiết, yêu cầu duyệt, pomodoro, thời gian dùng, âm thanh và hệ thống, mỗi thứ là một tab. Khi đóng, nó cho thấy bài đang phát và việc các tác tử lập trình của bạn đang làm.

![Notch mở lần lượt từng tab: clipboard, khay tệp, media, tập trung, tác tử AI với giới hạn sử dụng, một yêu cầu cấp quyền và lịch](docs/images/notch-tabs.gif)

### Lời chào của chú ma

Mỗi lần bạn tự mở Figghost, chú ma đi xuống từ notch, chớp mắt một lần rồi đi lên lại. Không bao giờ chạy khi đăng nhập hay khi ứng dụng khởi động nền, và có thể tắt ở trang Chung. Chú ma được vẽ bằng agy; còn có một [video](docs/images/ghost-hello.mp4).

![Chú ma đi xuống từ notch, chớp mắt một lần rồi đi lên lại](docs/images/ghost-hello.gif)

### Chia cửa sổ từ notch

Kéo cửa sổ bằng thanh tiêu đề lên mép trên, các bố cục treo dưới notch: một nửa, một phần ba, một phần tư, cửa sổ rộng cạnh cửa sổ hẹp, hoặc bố cục bạn tự vẽ từ lưới. Trỏ vào một bố cục thì các ô của nó hiện trên màn hình; đi xuống một ô rồi thả để đặt cửa sổ vào đó.

![Một cửa sổ được kéo lên mép trên: hàng bố cục hiện ra, trỏ vào bố cục một phần ba, một ô sáng lên dưới cửa sổ](docs/images/snap.gif)

### Chọn thông minh

Chọn chữ ở bất kỳ đâu rồi bấm phím tắt. Một cửa sổ nhỏ cạnh con trỏ cho biết đó là gì: một độ dài ở đơn vị khác, một khoản tiền theo các đồng tiền của bạn, một giờ ở các múi khác, hoặc một bản dịch.

![Chọn thông minh trả lời một độ dài, một khoản tiền, một giờ ở nhiều múi và một bản dịch](docs/images/smart-select.gif)

### Vòng kéo thả

Bắt đầu kéo một tệp rồi chạm một phím bổ trợ: một vòng mở ra quanh con trỏ với những việc có thể làm với tệp (đổi cỡ, nén, tách hoặc gộp PDF, chú thích, chép chữ, cắt chủ thể, xóa siêu dữ liệu). Thả vào một ô để chạy.

![Vòng kéo thả chín ô, một ô sáng dưới con trỏ](docs/images/drag-wheel.gif)

### Cửa sổ cài đặt theo phong cách macOS

Liquid Glass, cả hai giao diện sáng tối, sáu nhóm trên một dock, và mỗi tính năng đều tắt được ở trang riêng. Xem [Cửa sổ cài đặt](#cửa-sổ-cài-đặt).

## Tính năng chi tiết

Mọi hình ở đây đều do chính mã của ứng dụng vẽ ra, không cần màn hình (`app-ui/settings_preview`, `macos-sys/render_notch`, `render_snap`, `render_smart`, `render_wheel`, `render_hud`, xem [docs/development.md](docs/development.md)); ảnh hero được tạo bằng agy. Các GIF là khung hình của những bản vẽ đó, còn ảnh các trang cài đặt là giao diện tiếng Việt.

Mở ứng dụng là mở cửa sổ cài đặt; đóng cửa sổ thì ứng dụng về lại thanh menu. Các tính năng ghi "tắt mặc định" được bật ở trang riêng của nó trong phần cài đặt.

### Cuộn chuột (như Mos)

Cuộn bánh xe chuột mượt, đảo chiều chỉ cho chuột ngoài, phím bổ trợ (cuộn ngang, nhanh hơn, phóng to) và quy tắc riêng cho từng ứng dụng. Đường cong vẽ trong phần cài đặt khớp với việc bộ làm mượt thực sự làm.

![Trang Cuộn chuột với đường cong, tốc độ và quy tắc theo ứng dụng](docs/images/settings-scroll.png)

### Lịch sử clipboard (như Maccy)

Văn bản, liên kết, mã, ảnh và tệp; tìm kiếm tức thì; ghim kèm một chữ cái để dán; Copy Stack; các bảng đã ghim; dữ liệu nhạy cảm bị giữ ngoài hoặc che đi; một cửa sổ hiện tại con trỏ; và một máy chủ MCP để trợ lý AI đọc được lịch sử (tắt mặc định).

![Tab Clipboard của notch với bộ lọc và các thẻ văn bản, liên kết, mã, ảnh và tệp đã chép](docs/images/notch-clipboard.png)

Tab Khay tệp giữ các tệp được thả vào dưới dạng tham chiếu tới nơi chúng nằm. Nhấp chuột phải vào một tệp để nén ZIP, chuyển đổi, làm nhỏ, xóa nền hoặc chép chữ trong ảnh.

![Tab Khay tệp của notch với các tệp và thư mục](docs/images/notch-shelf.png)

### Notch (như OneNotch)

Bảy tab, và mười tab khác vào cùng khi bật tính năng của chúng (Hệ thống, Ghi chú, Mở nhanh, Thời tiết, Yêu cầu duyệt, Pomodoro, Thời gian dùng, Tiến độ ngày, Thiết bị và Màn hình), chia cửa sổ, chỉ báo âm lượng tùy chọn, lời bài hát, hoạt động trực tiếp trong khối đóng, và kích thước do bạn chọn (gọn, chuẩn, rộng hoặc tùy chỉnh). Notch hiện cả trên ứng dụng toàn màn hình, trừ khi tắt "Hiện trên ứng dụng toàn màn hình" ở trang Notch: khi đó nó biến mất hoàn toàn trong lúc ứng dụng phía trước đang toàn màn hình trên màn hình của nó, và quay lại, với các tab và hoạt động như cũ, khi ứng dụng đó thoát toàn màn hình hoặc ứng dụng khác lên trước.

![Tab Media của notch với ảnh bìa, tên bài và các nút phát](docs/images/notch-media.png)

Tab Hệ thống cho thấy bộ xử lý từng lõi (lõi tiết kiệm và lõi nhanh riêng), bộ nhớ chia theo ứng dụng, wired và nén cùng áp lực và swap, mạng với một phút lịch sử, bộ xử lý đồ họa, dung lượng còn trống của ổ khởi động, nhiệt độ bộ xử lý theo độ cùng trạng thái hệ thống báo, tốc độ quạt trên máy có quạt, và sức khỏe pin trên máy có pin. Tab tắt cho tới khi bật trong Cài đặt, mục Tab của notch, Hệ thống, nơi từng thẻ có thể bỏ đi. Nó chỉ đọc hệ thống khi tab đang hiện, mỗi giây một lần hoặc theo tần suất chọn, nên ứng dụng nhàn rỗi không tốn công cho nó.

![Tab Hệ thống của notch](docs/images/notch-system.png)

Tab Ghi chú là nơi nháp: danh sách ghi chú ở bên trái và trình soạn ở bên phải. Mỗi ghi chú là một tệp văn bản thuần trong một thư mục riêng (`notes` trong thư mục dữ liệu) nên ứng dụng khác đọc được (tệp dài hơn năm chục nghìn ký tự không được liệt kê, để không bao giờ bị cắt); chữ gõ được lưu một lát sau phím cuối, ghi chú xóa đi vào Thùng rác, và ghi chú để trống không để lại tệp. Tắt cho tới khi bật trong Cài đặt, mục Tab của notch, Ghi chú.

![Tab Ghi chú của notch](docs/images/notch-notes.png)

Tab Mở nhanh là lưới tối đa mười hai ô cho ứng dụng, thư mục và Phím tắt Shortcuts của Apple bạn ghim trong Cài đặt, mục Tab của notch, Mở nhanh. Một ô chỉ làm gì đó khi được bấm: ứng dụng được tìm theo bundle identifier ở nơi nó đang nằm, thư mục mở trong Finder, và một Shortcut chạy theo tên bằng lệnh `shortcuts` của hệ thống (tên truyền làm tham số, không bao giờ qua shell); notch biến đi sau đó. Tắt cho tới khi bật, và chưa ghim gì thì chưa có gì.

![Tab Mở nhanh của notch](docs/images/notch-launcher.png)

Tab Thời gian dùng cho thấy mỗi ứng dụng ở phía trước bao lâu hôm nay và trong bảy ngày qua, dưới dạng tổng và một danh sách có thanh cho từng ứng dụng, theo ứng dụng hoặc theo nhóm ứng dụng tự khai (Năng suất, Công cụ lập trình, Tiện ích...). Mỗi năm giây, khi có người ở máy, ứng dụng phía trước được cộng khoảng thời gian đó; thời gian ngừng đếm khi bàn phím và con trỏ đứng yên suốt thời gian bạn chọn (1, 2 hoặc 5 phút). Chỉ tên, bundle identifier của ứng dụng và số giây được giữ, trong ba mươi ngày, ở tệp `screentime.json` của thư mục dữ liệu: không tiêu đề cửa sổ, không chữ, không màn hình, không phím, không cần quyền, và không gì rời khỏi máy. Tắt cho tới khi bật trong Cài đặt, mục Tab của notch, Thời gian dùng.

![Tab Thời gian dùng của notch](docs/images/notch-screentime.png)

Tab Pomodoro xen kẽ phiên làm việc và giờ nghỉ: một vòng tròn đầy dần khi một giai đoạn trôi qua với đồng hồ ở giữa, các chấm của vòng cho biết đã xong bao nhiêu phiên, và một giờ nghỉ dài kết thúc mỗi vòng (độ dài và số phiên trong một vòng do bạn chọn). Khi một giai đoạn chạy, thời gian còn lại nằm trong notch đóng, và khi nó kết thúc có tiếng báo và notch mở ở tab đó vài giây (mỗi thứ tắt được; giai đoạn kế có thể tự bắt đầu). Tab đếm số phiên đã xong hôm nay, số ngày liên tiếp và bảy ngày qua, trong tệp `pomodoro.json` của thư mục dữ liệu trong ba mươi ngày, và một phím tắt bắt đầu hoặc tạm dừng từ mọi ứng dụng. Đây là một tab cạnh Tập trung, vẫn là đồng hồ đếm ngược đơn giản như trước, và tắt cho tới khi bật trong Cài đặt, mục Tab của notch, Pomodoro.

![Tab Pomodoro của notch](docs/images/notch-pomodoro.png)

Tab Yêu cầu duyệt liệt kê các yêu cầu duyệt đang chờ bạn trên GitHub và GitLab, mới nhất trước, với người viết và cách đây bao lâu, kèm một chấm trên biểu tượng tab khi có yêu cầu; bấm một dòng sẽ mở nó trên trình duyệt. Figghost không bao giờ thấy mật khẩu hay token của bạn: nó hỏi các công cụ dòng lệnh `gh` và `glab` mà bạn đã đăng nhập sẵn (tìm trong Homebrew và các chỗ thông dụng, chạy với tham số cố định và không qua shell), chỉ đọc những yêu cầu có tiêu đề, dự án, số, thời gian và địa chỉ https, chỉ mở đúng địa chỉ đó, và không ghi gì xuống đĩa. Nó xem sớm sau khi ứng dụng khởi động rồi theo khoảng bạn chọn (5, 10, 30 hoặc 60 phút) khi đang bật, và trang cài đặt cho biết từng công cụ đã cài và đăng nhập chưa. Tắt cho tới khi bật trong Cài đặt, mục Tab của notch, Yêu cầu duyệt.

![Tab Yêu cầu duyệt của notch](docs/images/notch-prs.png)

Tab Thời tiết cho thấy thời tiết của tối đa tám thành phố, mỗi lần một thành phố: hiện tại, sáu giờ tới và năm ngày tới, theo độ C hoặc độ F. Khi có nhiều thành phố, mũi tên cạnh tên (hoặc phím Trái và Phải) lật sang thành phố kế, và "2/3" cho biết đang ở đâu; tab mở ở thành phố đầu danh sách và nhớ thành phố đang xem khi ứng dụng chạy. Đây là tab duy nhất đi ra mạng, nên nó tắt cho tới khi bật và thêm thành phố trong Cài đặt, mục Tab của notch, Thời tiết, nơi thành phố được tìm, thêm, chuyển lên và xóa; cho tới lúc đó nút Mở cài đặt của nó dẫn tới đó. Dự báo lấy từ Open-Meteo, không cần tài khoản hay khóa, qua `curl` của hệ thống; thứ được gửi đi là tên gõ khi tìm thành phố rồi tọa độ của thành phố trên màn hình, không bao giờ là vị trí của máy Mac, và nó chỉ hỏi khi tab đang mở, chỉ cho thành phố đang hiện, và không quá một lần mỗi mười phút cho mỗi thành phố.

![Tab Thời tiết của notch](docs/images/notch-weather.png)

Tab Tiến độ ngày cho thấy ngày đã trôi qua bao nhiêu, dưới dạng một phần trăm lớn và một đường từ sáng đến giờ đi ngủ (7 giờ sáng và 11 giờ đêm, cả hai do bạn chọn). Các sự kiện hôm nay nằm dọc đường dưới dạng thanh theo màu lịch của chúng (sự kiện cả ngày bị bỏ qua), cái đang diễn ra sáng hơn, mỗi lời nhắc đến hạn vào một giờ trong ngày là một chấm trên đó, và bên dưới là việc đang diễn ra và việc kế tiếp cùng khoảng cách còn lại. Nó đọc các lịch mà tab Lịch đọc, bỏ những lịch tab đó bỏ, và không xin thêm gì: khi lịch chưa được cho phép nó nói ra thay vì hiện một ngày trống. Tắt cho tới khi bật trong Cài đặt, mục Tab của notch, Tiến độ ngày; nó xem ngày khi tab mở và lại mỗi nửa phút khi tab hiện, và không giữ gì trên đĩa.

![Tab Tiến độ ngày của notch](docs/images/notch-dayprogress.png)

Tab Thiết bị cho thấy mức pin của các thiết bị Bluetooth đang kết nối, mỗi thiết bị một thẻ với biểu tượng, tên và một đồng hồ đo: chuột, bàn phím, bàn di chuột, tay cầm, loa hoặc tai nghe, và một đôi tai nghe nhét tai với từng bên và hộp sạc. Thiết bị ở hoặc dưới mức bạn chọn (10, 15, 20 hoặc 30 phần trăm) có màu cảnh báo và chữ Thấp. Thiết bị chỉ có mức pin khi nó báo cho máy Mac, mà nhiều bàn phím, chuột và loa không báo; những thiết bị đó được liệt kê không kèm mức. Một số chỉ báo thỉnh thoảng (tai nghe có thể chỉ báo một lần khi kết nối), nên mức pin mà thiết bị đã thôi báo được giữ hai giờ, vẽ mờ hơn và ghi thời điểm báo. Máy Mac được hỏi qua `system_profiler` của chính nó (chạy với tham số cố định, không qua shell, ngoài luồng chính và dừng sau vài giây) khi tab mở và mỗi nửa phút khi tab hiện, và không bao giờ vào lúc khác. Không cần quyền và không lưu gì. Tắt cho tới khi bật trong Cài đặt, mục Tab của notch, Thiết bị.

![Tab Thiết bị của notch](docs/images/notch-devices.png)

Tab Màn hình đặt độ sáng và độ tương phản cho từng màn hình ngoài, như các nút của chính màn hình sẽ làm (như MonitorControl): một thẻ cho mỗi màn hình trả lời qua DDC/CI, với một thanh trượt cho mỗi thiết lập nó có. Chính màn hình thay đổi; không có gì phủ lên hình. Khi bật thêm phím độ sáng, chúng điều chỉnh mọi màn hình ngoài cùng lúc và hiện mức trong kiểu chỉ báo âm lượng, với mặt trời thay cho loa; trong lúc màn hình tích hợp còn sáng, các phím vẫn thuộc về nó, và Control cùng một phím sẽ đặt màn hình ngoài. Các màn hình được tìm khi tab mở và khi một phím cần đến, và chỉ độ sáng và độ tương phản được hỏi hoặc đặt. Tab không cần quyền, còn các phím cần Trợ năng. Màn hình sau hub, bộ chuyển DisplayLink hoặc một số cổng HDMI không trả lời và không được liệt kê. Tắt cho tới khi bật trong Cài đặt, mục Tab của notch, Màn hình.

![Tab Màn hình của notch](docs/images/notch-displays.png)

![Trang Notch của phần cài đặt với các kiểu kích thước và công tắc hành vi](docs/images/settings-notch.png)

Chỉ báo âm lượng có bảy kiểu, mỗi kiểu vẽ ở ba mức:

![Chỉ báo âm lượng các kiểu: notch, kính, đảo, bên cạnh, ô, thanh và góc](docs/images/volume-indicator.gif)

### Tác tử AI

Tab Tác tử cho thấy các phiên Claude Code, Codex và pi (đang làm, đang chờ, xong), giới hạn sử dụng của Claude Code và Codex kèm nút làm mới, và token cùng chi phí theo giá API. Một yêu cầu cấp quyền từ Claude Code hoặc Codex được trả lời ngay trên notch bằng Cho phép hoặc Từ chối, còn một câu hỏi có lựa chọn (AskUserQuestion của Claude Code, ask_user_question của pi) có một nút cho mỗi lựa chọn; notch đóng cho biết có câu hỏi đang chờ. Thẻ đứng yên trong lúc các phiên khác báo cáo, và khi con trỏ đang ở trên nó thì tác tử vẫn chờ câu trả lời; khi có nhiều yêu cầu mở, bấm vào dòng của một phiên đang chờ sẽ đưa yêu cầu của nó lên thẻ. Khi một tác tử xong hoặc lỗi, notch đóng nói rõ cái nào và ở đâu trong vài giây, ví dụ "Claude figghost" màu xanh hay "Pi shop" màu đỏ; tác tử xong không phát tiếng, và một phiên đã xong từ trước không báo lại. Trên notch đóng mỗi tác tử là một nhân vật pixel nhỏ, Claude Code là Clawd ở màn hình chào của nó, Codex là đám mây và pi là chữ pi của nó: gõ laptop hoặc cuốc ruộng khi đang làm, giơ dấu hỏi khi hỏi, nhảy cẫng lên khi xong và khóc khi lỗi.

pi được kết nối từ chính trang cài đặt đó: Figghost ghi một tiện ích mở rộng vào `~/.pi/agent/extensions/figghost.ts` để báo các phiên của pi và đưa câu hỏi của nó lên notch trong khi pi cũng hỏi ở terminal, nên ai trả lời trước thì thắng. Khi pi chạy Claude Code qua nhà cung cấp claude-bridge, các hook của Claude Code bên trong nó giữ im lặng, vì pi tự báo phiên đó.

Tài khoản: trang Tác tử AI của phần cài đặt liệt kê các tài khoản Claude Code và Codex bạn giữ trong Orca, với gói, tài khoản nào Orca đang dùng và tài khoản nào lệnh `claude` (hoặc `codex`) thường đang dùng, cùng giới hạn năm giờ và hằng tuần của mỗi tài khoản Claude. Nút Chuyển làm cho lệnh thường dùng một trong số đó, sau khi xác nhận: Figghost giữ một bản sao riêng (chế độ 0600, năm bản gần nhất) của thông tin đăng nhập và `oauthAccount` mà lệnh đang có, rồi chép thông tin đăng nhập của tài khoản vào mục Keychain `Claude Code-credentials` và `oauthAccount` của nó vào `~/.claude.json`, giữ nguyên mọi khóa khác tại chỗ (với Codex là `auth.json` của nó đè lên `~/.codex/auth.json`). Nút Khôi phục đặt lại bản sao mới nhất. Các phiên mới dùng tài khoản mới; phiên đang chạy có thể cần khởi động lại. Tài khoản được chuyển khi đó dùng chung một đăng nhập với Orca, nên khi một bên gia hạn thì bên kia có thể phải đăng nhập lại. Tab Tác tử của notch ghi nhãn các dòng dùng Claude bằng tài khoản mà lệnh `claude` đang dùng.

![Tab Tác tử với ba phiên, thanh giới hạn sử dụng và chi phí token](docs/images/notch-agents-overview.png)

![Yêu cầu cấp quyền từ Claude Code với nút Cho phép và Từ chối trên notch](docs/images/notch-agents-approval.png)

### Chia cửa sổ

Bố cục theo từng màn hình, làm từ mẫu có sẵn hoặc từ lưới, mở bằng cách kéo cửa sổ lên mép trên.

![Hàng bố cục dưới notch, bố cục một phần ba dưới con trỏ](docs/images/snap-zones.png)

![Các ô của bố cục một phần ba trên màn hình, ô giữa sáng lên nơi cửa sổ sẽ đặt vào](docs/images/snap-cells.png)

![Trang Chia cửa sổ với các bố cục và trình sửa lưới](docs/images/settings-snap.png)

### Luôn trên cùng

Một phím tắt giữ cửa sổ ở phía trước, trên mọi cửa sổ khác, với viền đỏ quanh nó (tắt mặc định). macOS chỉ cho một chương trình đưa lên cửa sổ của chính nó, nên khi cửa sổ khác che cửa sổ đã ghim, một hình ảnh trực tiếp của nó được hiện trên chỗ che, và nhấp vào hình sẽ đưa cửa sổ thật lên trước. Mỗi lần ghim một cửa sổ, từ phím tắt hoặc từ công tắc ở bảng thanh menu, và chính phím tắt đó thả nó ra. Các cửa sổ nổi như hình trong hình vẫn nằm trên nó, đúng như hệ thống chủ ý.

![Trang Luôn trên cùng với phím tắt, cửa sổ đã ghim và các quyền cần có](docs/images/settings-pin.png)

### Con trỏ

Cho con trỏ một diện mạo riêng (tắt mặc định): mũi tên cầu vồng có màu xoay, một trong sáu nhân vật pixel của linh vật clipboard biết đứng, chớp mắt và cười khi bạn trỏ vào liên kết, hoặc bất kỳ ảnh, GIF hay PNG động nào bạn chọn, với kích thước, tốc độ và điểm bấm riêng. Mũi tên và bàn tay trên liên kết thay đổi; con trỏ chữ vẫn của hệ thống. Hệ thống tự phát hoạt ảnh, nên không có gì chạy khi con trỏ di chuyển. Nó hiện trên mọi ứng dụng khác, và con trỏ trở lại bình thường khi bạn tắt hoặc thoát; nếu ứng dụng bị buộc dừng, tắt rồi bật lại tính năng sẽ trả con trỏ về.

![Trang Con trỏ với các hình để chọn](docs/images/settings-cursor.png)

### Âm thanh (như SoundSource và Boom 3D)

Âm lượng tới 400 %, tắt tiếng, thiết bị phát và equalizer cho từng ứng dụng, hiệu ứng trên mọi thứ đang phát (equalizer, compressor, limiter), và phím âm lượng cho loa mà hệ thống không giảm được.

### Lịch

Sự kiện và lời nhắc hôm nay trên notch, và báo trước cuộc họp với nút vào liên kết Zoom, Meet, Teams hoặc Webex (tắt mặc định).

![Tab Lịch của notch với các sự kiện trong ngày](docs/images/notch-calendar.png)

### Chọn thông minh và dịch

Đơn vị, tiền tệ, giờ và bản dịch cho chữ đang chọn (tắt mặc định). Tỷ giá là một bảng tải về một lần; bản dịch gửi tối đa 450 ký tự tới một dịch vụ công cộng, và chỉ khi bạn yêu cầu.

![Chọn thông minh trả lời 5 km ra dặm và feet](docs/images/smart-units.png)

![Chọn thông minh trả lời một khoản tiền với tỷ giá](docs/images/smart-money.png)

![Chọn thông minh cho thấy một giờ ở các múi khác](docs/images/smart-times.png)

![Chọn thông minh dịch một cụm từ](docs/images/smart-translation.png)

### Gõ tiếng Việt (như Gõ Nhanh)

Telex hoặc VNI ở mọi ứng dụng mà không cần bộ gõ nào khác, qua bộ máy của [Gõ Nhanh](https://github.com/khaphanspace/gonhanh.org) (tắt mặc định). Dấu được đặt theo quy tắc mới (`hoà`, `khoẻ`) hoặc cách cũ, từ tiếng Anh trở lại khi hết từ (`text`, `with`), một phím tắt (mặc định Control+Space, do chính ứng dụng ghi để cách hệ thống dùng cùng phím không lấy mất được) hoặc bảng thanh menu chuyển sang tiếng Anh và ngược lại kèm một chữ trên notch, VI hoặc EN nằm sẵn trên notch đóng và trong một mục riêng nhỏ ở thanh menu (bấm vào để chuyển; mỗi nơi tắt được), và các ứng dụng bạn liệt kê được để yên. Thanh địa chỉ, ô tìm kiếm, Spotlight (ghi qua Trợ năng, dự phòng bằng phím), trình soạn dựng trên web view, terminal và ứng dụng Adobe, mỗi thứ nhận cách gõ mà Gõ Nhanh đã chốt cho nó. Khi một bộ gõ tiếng Việt khác đang chạy, Figghost để yên mọi phím.

### Chụp màn hình (như CleanShot X)

Chụp màn hình, chụp cuộn, quay màn hình (MP4 hoặc GIF, kèm hiển thị phím, âm thanh của máy và micro), trình chỉnh sửa, nhận dạng chữ tiếng Việt và tiếng Anh, và các phím tắt thao tác nhanh sau khi chụp (tắt mặc định).

![Trang Chụp màn hình của phần cài đặt](docs/images/settings-capture.png)

### Chọn màu

Kính lúp trên màn hình đóng băng; màu được chép dạng HEX, RGB, HSL hoặc HSB (tắt mặc định).

### Mole

Kiểm tra sức khỏe máy Mac và giải phóng dung lượng, theo các tính năng của [Mole](https://github.com/tw93/Mole) nhưng do chính Figghost làm nên không cần cài gì thêm: bộ nhớ đệm và nhật ký, thư mục build cũ của dự án, tệp cài đặt, ứng dụng cùng những gì nó để lại, và cái gì chiếm chỗ trên ổ đĩa, hiện thành biểu đồ vòng để rê chuột và bấm lát mở thư mục. Danh sách tự hiện ra khi mở trang; chọn từng dòng hoặc chọn tất cả rồi xóa cùng lúc. Bộ nhớ đệm và thư mục build bị xóa hẳn, còn lại được chuyển vào Thùng rác (tắt mặc định).

![Trang Mole với điểm sức khỏe và các danh sách để dọn](docs/images/settings-mole.png)

### Đảo trạng thái

Thông báo nhỏ ló ra từ notch đóng trong chốc lát: Caps Lock bật hoặc tắt, cắm hoặc rút sạc, pin đầy (ở mức bạn chọn) hoặc yếu, ổ đĩa cắm vào hoặc rút ra (và một mục Eject cho mỗi ổ ở bảng thanh menu), một tệp tải xong trong thư mục Downloads, và một ứng dụng bắt đầu dùng camera hoặc micro (mỗi thứ có công tắc riêng). Mỗi loại tắt được riêng và xem được dưới dạng mẫu; không gì đọc những gì trên màn hình hay những gì được gõ (tắt mặc định).

![Notch đóng với mười tám đảo trạng thái: Caps Lock, nguồn, pin, ổ đĩa, tải xuống, camera và micro, giữ máy thức, micro bị tắt tiếng và khóa bàn phím](docs/images/live-islands.png)

![Trang Đảo trạng thái của phần cài đặt](docs/images/settings-islands.png)

### Tiện ích

Giữ máy thức từ 15 phút đến 4 giờ hoặc cho tới khi bạn dừng, tắt tiếng chính micro để mọi ứng dụng không nghe thấy gì (nó được bật lại khi Figghost thoát, vì không có gì khác trên máy có công tắc cho việc này), khóa bàn phím trong lúc bạn lau (Command và Escape mở khóa ngay, và nó tự mở khóa sau thời gian bạn đặt), và tạm ẩn notch từ 5 phút đến 2 giờ hoặc cho tới khi bạn đánh thức, để nó không mở khi con trỏ chạm mép trên màn hình lúc bạn trình chiếu hoặc chơi game (một cú nhấp vào nó, hoặc một tác tử lập trình đang chờ câu trả lời, vẫn mở được). Mỗi thứ có chỗ ở bảng thanh menu (một công tắc cho giữ máy thức và một cho tạm ẩn notch, bắt đầu theo độ dài bạn chọn ở trang này; danh sách độ dài nằm ở menu), một phím tắt dùng ở mọi ứng dụng và một thông báo trên notch (tắt mặc định).

![Trang Tiện ích bằng tiếng Việt](docs/images/settings-utilities-vi.png)

### Và còn lại

- Linh vật clipboard: một nhân vật nhỏ diễn một câu chuyện mỗi lần có thứ được chép (tắt mặc định).
- Biểu tượng thanh menu: giấu những cái bạn không cần sau một vạch ngăn (tắt mặc định).
- Kéo cửa sổ vào notch để chia (tắt mặc định).
- Cập nhật: Figghost tìm bản mới mỗi ngày một lần và hỏi trước khi cài; một cửa sổ nói có gì mới sau khi cập nhật.

## Cài đặt

Máy Mac Apple silicon với macOS 13 trở lên. Với Homebrew:

    brew tap eliasbui/figghost
    brew install --cask eliasbui/figghost/figghost

Không dùng Homebrew:

    curl -fsSL https://raw.githubusercontent.com/eliasbui/figghost/main/scripts/install.sh | sh

Viết đầy đủ tên cask sẽ cài từ tap và chỉ tin đúng cask đó, nên không cần `brew trust`. Cả hai cách đều đặt `Figghost.app` vào `/Applications` (script nhận `DEST=~/Applications` nếu muốn chỗ khác). Ứng dụng được ký ad hoc và chưa notarize, nên cả hai cách đều gỡ cờ cách ly sau khi cài, như các ứng dụng notch mã nguồn mở khác; hãy tự build (bên dưới) nếu bạn không muốn vậy. `brew uninstall --cask figghost` gỡ ứng dụng, và thêm `--zap` sẽ xóa luôn dữ liệu trong `~/Library/Application Support/Figghost`. Figghost tìm bản mới mỗi ngày một lần và hỏi trước khi cài, kèm những gì mới; khi bật *Tự động cập nhật* ở trang Giới thiệu thì nó cài mà không hỏi, và *Kiểm tra* ở đó làm ngay. Một bản cập nhật thay ứng dụng tại chỗ, xóa bản cũ và tệp tải về, và có hiệu lực khi Figghost được khởi động lại (cửa sổ thông báo có nút cho việc đó). Nó cần ghi được vào thư mục chứa ứng dụng, điều mà quản trị viên làm được với `/Applications`; nơi không ghi được thì trang Giới thiệu dẫn tới bản phát hành, và chạy lại lệnh cài vẫn dùng như trước. Quyền Trợ năng được giữ qua các bản cập nhật, vì ứng dụng được ký bằng một tên mà mọi bản build đều chung; bản cập nhật từ một phiên bản từng ký riêng cho từng bản build sẽ hỏi lại một lần cuối (xóa Figghost cũ khỏi danh sách trong Cài đặt Hệ thống, Quyền riêng tư & Bảo mật, Trợ năng, rồi cấp lại).

## Build và chạy

Rust được cài bằng rustup vào `~/.cargo`. Giữ thư mục build bên ngoài dự án:

    . "$HOME/.cargo/env"
    export CARGO_TARGET_DIR="$HOME/Library/Caches/figghost-target"
    cargo test --workspace
    cargo clippy --workspace --all-targets -- -D warnings
    scripts/bundle.sh            # tạo dist/Figghost.app
    scripts/release.sh 0.2.1     # phát hành và trỏ cask Homebrew tới bản đó (DRY_RUN=1 để kiểm tra trước)

Để phát triển, tạo một chứng chỉ Code Signing cục bộ bền vững trong Keychain Access (ví dụ `Figghost Local`) và cài trình theo dõi một lần bằng `cargo install cargo-watch`. Sau đó `SIGN_IDENTITY="Figghost Local" scripts/dev.sh` build lại và khởi động lại ứng dụng cục bộ mỗi khi tệp nguồn đổi. Danh tính ổn định giữ quyền Trợ năng qua các lần build; dừng trình theo dõi bằng Control-C.

Quyền Trợ năng gắn với chữ ký mã của ứng dụng. Chữ ký ad hoc mặc định đặt tên cho ứng dụng (`vn.figghost.app`) thay vì cho một bản build, nên build lại hoặc cập nhật vẫn giữ quyền đã cấp. Điều đó chỉ mạnh bằng cái tên: một chương trình tự ký bằng cùng tên cũng được cho vào. Để gắn quyền vào một chứng chỉ không ai khác có, tạo một chứng chỉ ký mã cục bộ trong Keychain rồi chạy `SIGN_IDENTITY="Tên chứng chỉ" scripts/bundle.sh`; đổi từ loại chữ ký này sang loại kia sẽ hỏi lại quyền một lần. `scripts/dev.sh` nhận `SIGN_IDENTITY=-` cho chữ ký ad hoc.

Quyền Trợ năng cần cho cuộn mượt, dán tự động, chia cửa sổ, giữ cửa sổ luôn trên cùng và chỉ báo âm lượng. Không có nó ứng dụng vẫn chạy: cửa sổ clipboard và notch hoạt động, các mục được chép để dán thủ công, và bộ cuộn tự khởi động khi quyền được cấp. Music và Spotify hỏi một lần xin phép để tab Media đọc được.

Biểu tượng ứng dụng được vẽ bằng mã: `cargo run -p macos-sys --example make_icon -- X.iconset`, rồi `iconutil -c icns X.iconset -o crates/app-ui/assets/Figghost.icns`.

## Windows và Linux (beta)

Figghost cho Windows 10, 11 và Linux đang là **bản beta**. Đây là một chương trình riêng, `figghost` (`crates/desktop`), dùng chung phần Rust đã có kiểm thử với bản macOS (`core-clipboard`, `core-agents`, `core-notes`): một biểu tượng ở khay hệ thống và một cửa sổ là trang web chạy trong web view của hệ thống (WebView2 trên Windows, WebKitGTK trên Linux). Từ 2.0.4, mỗi [bản phát hành trên GitHub](https://github.com/eliasbui/figghost/releases/latest) có kèm bản beta cho Windows (`figghost-<version>-windows-x86_64.zip`) và cho Linux (`figghost-<version>-linux-<arch>.tar.gz`); giải nén rồi chạy `figghost`. Các bản này chưa có chữ ký và chưa có bộ cài: Windows SmartScreen sẽ hỏi một lần, còn trên Linux cần cài các thư viện WebKitGTK, GTK và AppIndicator trong danh sách build bên dưới (bỏ hậu tố `-dev`). Hoặc build từ mã nguồn như bên dưới.

Có trong bản beta:

- Biểu tượng và menu ở khay hệ thống; đóng cửa sổ là cửa sổ được giải phóng hẳn, nên app nằm ở khay tốn rất ít tài nguyên.
- Lịch sử clipboard: chữ, liên kết, mã và tệp, và ảnh trên Windows. Ô tìm kiếm nhận cùng bộ lọc như trên macOS (`@link`, `@code`, `@text`, `@image`, `@file`, `@pin`), có ghim và xoá từng mục, và một phím tắt toàn cục (mặc định `ctrl+alt+KeyV`) để mở lịch sử. Nội dung trông giống mật khẩu hoặc khoá được ẩn và xoá sau một giờ, hoặc không bao giờ giữ, hoặc giữ, tuỳ lựa chọn; trên Windows, nội dung trình quản lý mật khẩu đánh dấu riêng tư không bao giờ được đọc.
- Ghi chú, mỗi ghi chú là một tệp văn bản trong thư mục `notes` của thư mục dữ liệu; ghi chú bị xoá được chuyển vào thùng rác.
- AI agents: cài và gỡ hook của Claude Code và Codex (và pi, trên Linux), xem các phiên làm việc và trạng thái của từng phiên, cho phép hoặc từ chối các lệnh đang chờ quyền ngay trong cửa sổ.
- MCP server, `figghost --mcp`, tắt cho đến khi bật trong cài đặt.
- Mở khi đăng nhập, tiếng Anh và tiếng Việt, giao diện sáng và tối.

Chưa có trong bản beta: notch và các tab, cuộn mượt, snap cửa sổ, âm thanh theo từng app, chụp màn hình, Smart Select, drag wheel, gõ tiếng Việt và các tính năng macOS khác. Mỗi tính năng cần gì trên Windows và Linux được ghi trong [docs/research-linux-windows.md](docs/research-linux-windows.md).

Giới hạn đã biết của bản beta:

- Clipboard được đọc hai lần mỗi giây, không theo từng thay đổi.
- Trên Linux, lịch sử giữ chữ và tệp, chưa giữ ảnh, và nội dung trình quản lý mật khẩu đánh dấu bí mật không được nhận ra (cài đặt cho mật khẩu là lớp bảo vệ).
- Trên Wayland, phím tắt toàn cục chỉ chạy khi một app X11 đang được focus: hãy gán một phím trong cài đặt của desktop để chạy lệnh `figghost`, vì lần chạy thứ hai sẽ hiện cửa sổ của app đang chạy. GNOME chỉ hiện biểu tượng ở khay khi có extension AppIndicator.
- Trên Windows, Claude Code chạy hook bằng Git Bash (vốn là yêu cầu của Claude Code); pi chưa hỗ trợ trên Windows.

Build trên Linux (tên gói của Debian và Ubuntu):

    sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev libxdo-dev libsqlite3-dev
    cargo build --release -p figghost-desktop        # target/release/figghost

Build trên Windows, với Rust cho MSVC (WebView2 có sẵn trên Windows 11; trên Windows 10 có thể phải cài thêm):

    cargo build --release -p figghost-desktop        # target\release\figghost.exe

`figghost --background` mở app ở khay mà không hiện cửa sổ. Dữ liệu nằm ở `%APPDATA%\Figghost` trên Windows và `$XDG_DATA_HOME/figghost` (`~/.local/share/figghost`) trên Linux, cùng các tệp như trên macOS (`config.json`, `history.sqlite`, `images`, `notes`); biến `FIGGHOST_HOME` đổi sang chỗ khác. `scripts/check-desktop-linux.sh` build và chạy test bản beta trong Docker, mở app trên một màn hình ảo và chụp ảnh cửa sổ; thêm `--windows` để build cả `figghost.exe` bằng MinGW.

## Thiết kế

Cửa sổ cài đặt có một thế giới riêng bên trên thế giới của hệ thống: đêm của chú ma. Diện mạo của nó theo những gì các ứng dụng đoạt giải Apple Design Award 2026 có chung, một nơi tự dựng mà mọi phần của cửa sổ đều thuộc về, với các điều khiển của nền tảng bên trong. Bầu trời phía sau trang đổi theo giờ (bình minh, ban ngày, hoàng hôn, và đêm có sao ở giao diện tối); tiêu đề, tên các mục và lời chú ma nói được viết tay bằng Patrick Hand, phông không dùng cho thứ gì khác; chú ma là người dẫn đường của mọi trang (một bong bóng nói trang để làm gì, nhảy lên và nói thêm một câu khi được bấm), là dấu hiệu cho biết ứng dụng đã có quyền cần thiết chưa (thức, hoặc ngủ với mắt nhắm, và chữ nói giống vậy) và là nhân vật của mọi danh sách trống, còn chú lớn ở phần đầu trang Chung nhìn theo con trỏ bằng mắt. Không thứ gì trong đó là dấu hiệu duy nhất của điều gì, nên nó dùng được cho người không phân biệt được màu, và mọi hoạt ảnh dừng khi bật Giảm chuyển động. [docs/design.md](docs/design.md) nói nó được làm thế nào và thêm vào ra sao.

![Trang Chung ở giao diện tối vào ban đêm, với chú ma cạnh tiêu đề trang và ở phần đầu](docs/images/ghost-world-general-dark.png)

![Trang Chung ở giao diện sáng ban ngày](docs/images/ghost-world-general-light.png)

Bản thân cửa sổ là một cửa sổ macOS trên Liquid Glass. Một dock kính nổi ở chân cửa sổ giữ một biểu tượng cho mỗi nhóm trong sáu nhóm trang, là các SF Symbol trắng trên ô màu, với một thấu kính đi theo cú bấm và kéo; ô tìm nằm giữa thanh tiêu đề; nội dung là các thẻ kính của những mục gom nhóm, mỗi hàng một nhãn và điều khiển của nó, trên một nền mềm, và các trang Chung và Giới thiệu mở bằng một thẻ đầu trang màu xanh của ứng dụng. Các điều khiển theo hệ thống: chúng lấy màu nhấn mà người dùng chọn trong Cài đặt Hệ thống, một trạng thái lấy màu hệ thống của riêng nó (xanh lá khi thứ gì đó bật hoặc được cấp), nút cao 24 điểm còn nút bật xuống và ô nhập cao 24 hoặc 28, tiêu đề mục đậm 13 pt, và mọi biểu tượng, trong cửa sổ này và trong bảng thanh menu, là SF Symbol do hệ thống vẽ. Trong notch, chữ màu trắng ở ba mức trên nền đen, mức mờ nhất vẫn đạt 5,3:1; một trang chỉ hiện gợi ý phím khi notch đang có bàn phím, và trên màn hình có khuyết camera thì phần đầu nằm cạnh khuyết khi vừa, nên hình khối mở ra không có dải trống ngang phía trên. Chỉ thứ người dùng chạm vào mới chuyển động, ngắn gọn để phản hồi và bằng lò xo cho thứ di chuyển. Độ tương phản được đo chứ không đoán: chữ ít nhất 4,5:1 và viền của điều khiển đang tắt ít nhất 3:1 trên những nền tệ nhất, ở cả hai giao diện. Mọi hiệu ứng đều có phương án dự phòng cho Giảm độ trong suốt, Tăng độ tương phản và Giảm chuyển động, tiêu điểm luôn nhìn thấy, mọi điều khiển đều có tên, và cả cửa sổ dùng được bằng bàn phím. Mọi token là biến CSS ở đầu `crates/app-ui/web/app.css`.

## Cấu hình

`~/Library/Application Support/Figghost/config.json`, do cửa sổ cài đặt ghi. Giá trị thiếu dùng mặc định, giá trị ngoài khoảng được kẹp lại, và tệp không đọc được bị chuyển thành `config.json.bad` thay vì bị ghi đè. Tệp do các phiên bản cũ ghi vẫn nạp nguyên. Mọi khóa, mọi giá trị chấp nhận được, cùng các biến môi trường và tham số mà các bản chạy thử dùng nằm trong [docs/configuration.md](docs/configuration.md).

## Dữ liệu và riêng tư

Lịch sử nằm trong `~/Library/Application Support/Figghost` (`history.sqlite`, thư mục `images`, `shelf.json` và các tệp của những tính năng bạn bật), với thư mục chế độ 0700 và không mã hóa. Ảnh chụp và bản quay được lưu vào thư mục ảnh chụp riêng của macOS (nếu không thì Desktop) trừ khi bạn chọn chỗ khác; phần hiển thị phím của bản quay cho thấy phím tắt và các phím như Return, không bao giờ là chữ gõ thường. Các mục do ứng dụng nguồn hoặc trình quản lý mật khẩu đánh dấu bị bỏ qua, nhưng một mật khẩu chép từ nguồn khác vẫn có thể bị lưu: hãy dùng danh sách ứng dụng bị bỏ qua hoặc xóa lịch sử. Gõ tiếng Việt đọc từng lần bấm phím bên trong ứng dụng để đặt dấu và không giữ gì ngoài từ đang gõ, trong bộ nhớ. Trang cài đặt được nạp từ một chuỗi và không thể ra mạng. Mọi thứ còn lại ở lại trên máy Mac này.

Ngoài lần xem bản mới mỗi ngày (một yêu cầu tới api.github.com không mang gì về bạn, và việc tải từ github.com khi bạn cập nhật), không có gì được gửi qua mạng trừ khi bạn bật, và khi đó chỉ đúng những gì chữ trong phần cài đặt nói: Chọn thông minh tải một bảng tỷ giá (không gì về bạn) và, để dịch, gửi chữ đang chọn (tối đa 450 ký tự) tới api.mymemory.translated.net; lời bài hát gửi tên nghệ sĩ, bài và album cùng độ dài bài tới lrclib.net. Cả hai đi qua curl của hệ thống bằng https. Nút làm mới giới hạn sử dụng ở tab Tác tử (Command+R ở đó) chạy chương trình Codex cài trên máy Mac này để hỏi giới hạn hiện tại; Codex tự liên hệ OpenAI, như mỗi khi bạn dùng nó, còn Figghost không gửi gì và không bao giờ đọc đăng nhập Codex. Nút làm mới của mục Tài khoản ở trang Tác tử AI đọc đăng nhập của từng tài khoản Claude trong Orca từ Keychain (macOS hỏi trước) và gửi access token của nó tới api.anthropic.com để hỏi giới hạn của tài khoản đó, như Orca và Claude Code làm; sau lần bấm đầu nó hỏi lại nhiều nhất mỗi năm phút khi trang đó đang mở. Token được truyền cho curl qua đầu vào chuẩn, không bao giờ qua dòng lệnh, token hết hạn không bao giờ được gia hạn, và không gì về đăng nhập được ghi log hay ghi ở đâu ngoài các bản sao riêng mà một lần chuyển giữ trong `account-backups` của thư mục dữ liệu. Các quyền được xin là Trợ năng (phím tắt tác động lên ứng dụng khác, chỉ báo âm lượng, Copy Stack, Chọn thông minh, kéo vào notch, giữ cửa sổ luôn trên cùng, gõ tiếng Việt), Ghi màn hình (chụp, quay, chọn màu và hình của cửa sổ được giữ trên cùng, chỉ khi bạn dùng), Ghi âm thanh hệ thống (Âm thanh, lần đầu đổi một ứng dụng, bản quay kèm âm thanh của máy, và các thanh của notch đóng khi chúng chạy theo âm thanh), Micro và Camera (một bản quay, khi bật trong tùy chọn của nó), Lịch và Lời nhắc (chỉ khi bạn bật lịch) và Tự động hóa Music và Spotify (tab Media).

## Lựa chọn công nghệ

Lớp vỏ dùng AppKit qua `objc2`, không dùng GPUI hay Tauri. Ô tìm kiếm phải nhận được kiểu gõ Telex và VNI, điều `NSSearchField` xử lý sẵn. Panel không kích hoạt, theo dõi chuột và vòng đời ứng dụng đã được kiểm chứng trong kho này, và bộ nhớ nhàn rỗi vẫn thấp. Cửa sổ cài đặt là một WKWebView vì một màn hình cài đặt có đồ thị trực tiếp, hiệu ứng kính và hai ngôn ngữ làm và tinh chỉnh nhanh hơn nhiều dưới dạng một trang, và trang chỉ là một máy khách mỏng: mọi quy tắc nằm trong mã Rust có kiểm thử. SQLite là thư viện của hệ thống (với FTS5 và bộ tách trigram), không phải bản đóng kèm.

## Bước tiếp theo

1. Thử các mục chưa kiểm chứng trong [docs/verification.md](docs/verification.md) trên phần cứng thật, bắt đầu từ những mục cần đầu vào thật hoặc màn hình thật: Chọn thông minh và chụp màn hình từ phím tắt, chọn màu, Copy Stack, kéo cửa sổ vào notch, biểu tượng thanh menu, một cảnh báo lịch, lời bài hát với bài đang phát, phím âm lượng vật lý, gõ Telex và VNI, màn hình có khuyết, mở khi đăng nhập.
2. Trong danh sách của OneNotch, còn lại: xóa đối tượng, hoạt động trực tiếp của Tập trung, hiện diện trên màn hình khóa, cử chỉ trên notch và notch trên mọi màn hình. Những gì các ứng dụng notch khác có mà Figghost chưa có, kèm điều khả thi và thứ tự làm, nằm trong [docs/feature-gap.md](docs/feature-gap.md) (bằng tiếng Việt).
3. Chữ ký Developer ID và notarization, cũng sẽ gắn quyền vào một chứng chỉ thay vì vào tên ứng dụng.
4. Nên lấy gì từ Raycast và PowerToys, và cái nào khả thi trên macOS từ một ứng dụng ký ad hoc, nằm trong [docs/research-raycast-powertoys.md](docs/research-raycast-powertoys.md).

## Phát triển

Cấu trúc kho, các chương trình trong `crates/*/examples` dùng để kiểm tra ứng dụng, và cách thêm một ngôn ngữ nằm trong [docs/development.md](docs/development.md). Những gì đã kiểm chứng và chưa kiểm chứng nằm trong [docs/verification.md](docs/verification.md).

## Giấy phép

MIT, xem [LICENSE](LICENSE).

Cửa sổ cài đặt dùng SF Pro, phông của chính hệ thống, nên đọc như phần còn lại của macOS. Inter được đóng kèm làm phông dự phòng cho máy không có nó, và Patrick Hand cho nét chữ tay của tiêu đề, tên mục và lời chú ma, cả hai theo SIL Open Font License 1.1: xem
[crates/app-ui/assets/Inter-OFL.txt](crates/app-ui/assets/Inter-OFL.txt) và
[crates/app-ui/assets/PatrickHand-OFL.txt](crates/app-ui/assets/PatrickHand-OFL.txt).

Gõ tiếng Việt đặt dấu bằng bộ máy Telex và VNI của [Gõ Nhanh](https://github.com/khaphanspace/gonhanh.org) (crate `gonhanh-core`, dùng nguyên bản), bản quyền 2025 Gõ Nhanh Contributors, theo giấy phép BSD 3-Clause: xem [crates/app-ui/assets/GoNhanh-LICENSE.txt](crates/app-ui/assets/GoNhanh-LICENSE.txt), tệp này cũng đi kèm trong ứng dụng. Figghost không do tác giả của Gõ Nhanh làm ra hay bảo trợ.
