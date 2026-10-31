# Quản trị mạng: sơ đồ nối dây giữa đường mạng và app

Ghi chép trước khi làm. Yêu cầu: một popup canvas dạng sơ đồ. Bên trái là các đường Internet đang có (Ethernet, Wi-Fi, điện thoại cắm USB). Ở giữa là các nút gộp: một đường đơn, hoặc nút "tổng" ghép nhiều đường (net-merge). Bên phải là các app đang dùng mạng. Người dùng kéo dây từ đường mạng hay nút tổng sang app để chọn app đó đi đường nào, đặt giới hạn tốc độ cho từng app, xem app nào đang dùng bao nhiêu. App nối ở "dây sau" thì được ưu tiên thấp hơn app chính. Bản beta: chia mạng của Mac sang điện thoại qua cáp USB.

Đã đọc `crates/core-netmerge`, trang Tải video, tab System, `scripts/bundle.sh`, và chạy thử `nettop` trên máy này. Chưa viết dòng mã nào.

## Figghost đã có gì

- `core-netmerge` (2042 dòng, có test): liệt kê đường mạng (`links()`: tên interface, chỉ số, IPv4, nhãn "Wi-Fi", "Ethernet", "iPhone USB"), dò đường nào ra được Internet (TCP tới 1.1.1.1 và 8.8.8.8 cổng 443), ghim một kết nối vào một đường bằng `IP_BOUND_IF`, chia kết nối theo trọng số tốc độ đo được, tránh đường làm hỏng chứng chỉ của một site, tạm dừng khi VPN chở toàn bộ lưu lượng. Đo tốc độ và tổng byte theo từng đường (`LinkStats`). Không cần quyền quản trị.
- Giới hạn của nó: chỉ lưu lượng đi qua proxy SOCKS5 trên 127.0.0.1 mới được chia, và hiện chỉ yt-dlp được cấu hình đi qua. App khác trên máy không biết proxy đó.
- Tab System đã đọc tốc độ mạng tổng, có một phút lịch sử. Nghiên cứu iStat Menus (`research-istat-menus-7.md`) đã ghi băng thông theo app là việc còn thiếu.
- Ứng dụng đang ký ad hoc, không notarize (`scripts/bundle.sh`, `scripts/install.sh`). Đây là ràng buộc lớn nhất của cả tính năng, xem dưới.

## Từng phần làm được đến đâu trên macOS

| Phần | Cách làm | Cần gì | Mức khả thi |
|---|---|---|---|
| Liệt kê đường mạng | `core-netmerge::links()` đã có | Không | Có sẵn |
| Theo dõi app nào dùng bao nhiêu, qua đường nào | `nettop -P -L 1 -x -J bytes_in,bytes_out` cho tổng theo tiến trình; `nettop -n -m tcp` cho từng socket kèm cột `interface` (`en0`, `en7`...) | Không cần root, đã chạy thử được trên máy này | Làm ngay được |
| Chọn app đi Ethernet hay Wi-Fi | Transparent proxy của NetworkExtension (`NETransparentProxyProvider`): hệ thống đưa từng luồng TCP và UDP của mọi app vào extension, kèm danh tính app (`sourceAppSigningIdentifier`). Extension mở kết nối ra ngoài bằng `NWConnection` với `requiredInterface` là đường đã nối cho app đó | System Extension, entitlement `app-proxy-provider-systemextension`, ký Developer ID, notarize, người dùng bấm cho phép trong System Settings | Làm được, nhưng phải đổi cách phát hành |
| Nút "tổng" (gộp Ethernet + Wi-Fi) | Bộ lập lịch của `core-netmerge` chạy trong extension thay vì sau proxy SOCKS: mỗi luồng mới chọn một đường theo trọng số | Như trên | Làm được. Một kết nối đơn vẫn chỉ nhanh bằng một đường; chỉ app mở nhiều kết nối (trình duyệt, trình tải, Steam, cập nhật hệ thống) mới thấy tốc độ cộng lại |
| Gộp thật cho một kết nối đơn | MPTCP hoặc bonding qua một máy chủ trung gian (cách Speedify làm) | Một VPS chạy phía bên kia, và chi phí vận hành | Ngoài phạm vi; ghi lại làm lựa chọn sau |
| Giới hạn tốc độ từng app | Extension là nơi chuyển tiếp dữ liệu, nên giới hạn bằng token bucket khi đọc và ghi; TCP tự giảm tốc theo áp lực ngược | Như trên | Làm được. Cách thay thế không cần extension là `pf` + `dnctl` (dummynet), nhưng nó cần root và chỉ khớp theo cổng, địa chỉ, không theo app |
| Ưu tiên app chính, app "dây sau" chịu thiệt | Hàng đợi có trọng số trong extension: khi đường bão hòa, luồng của app ưu tiên thấp được cấp phần nhỏ hơn, hoặc bị giới hạn mềm | Như trên | Làm được, chỉ hiệu quả khi đường thật sự bão hòa |
| Chia mạng Mac sang điện thoại Android qua USB | Reverse tethering kiểu gnirehtet: `adb reverse` + một app VPN trên điện thoại chuyển gói về Mac; không cần root ở cả hai đầu | `adb`, bật gỡ lỗi USB, cài app phía Android (Android 5.0 trở lên); chỉ IPv4 | Khả thi cho beta. gnirehtet không còn được bảo trì tích cực, nên hoặc gói bản Rust của nó (Apache 2.0), hoặc viết lại phần relay |
| Chia mạng Mac sang iPhone qua USB | Internet Sharing của macOS, chọn đích "iPhone USB" | Bật Personal Hotspot trên iPhone; bật Internet Sharing cần quyền quản trị và không có API công khai để bật bằng mã | Không ổn định theo báo cáo người dùng (mục "iPhone USB" có lúc biến mất). Figghost chỉ nên hướng dẫn và mở đúng trang System Settings |

Chiều ngược lại (điện thoại phát mạng cho Mac qua USB) đã chạy với net-merge: interface "iPhone USB" hiện ra như một đường mạng.

## Ràng buộc quyết định: System Extension

Mọi thứ ngoài "theo dõi" đều cần can thiệp vào lưu lượng của app khác. Trên macOS hiện đại, cách được hỗ trợ duy nhất là NetworkExtension. Kernel extension đã bị loại bỏ; `pf` thì không biết app nào.

Điều đó kéo theo:

1. Tài khoản Apple Developer trả phí, chứng chỉ Developer ID, notarize mọi bản phát hành. Bản ký ad hoc không nạp được System Extension khi SIP bật.
2. Thay đổi `bundle.sh` và `release.sh`: thêm một bundle `.systemextension` trong `Contents/Library/SystemExtensions`, file entitlements cho cả app và extension, bỏ bước gỡ cờ quarantine trong cask.
3. Extension là một tiến trình riêng do hệ thống quản lý. Viết bằng Rust qua `objc2` được, vì `NETransparentProxyProvider` là lớp Objective-C, nhưng phải định nghĩa lớp con trong Rust. Mẫu và tài liệu đều bằng Swift; một extension nhỏ bằng Swift gọi lõi Rust (bộ lập lịch, token bucket, luật) qua FFI là đường ít rủi ro hơn.
4. Rủi ro vận hành: extension lỗi thì app nào đang bị chuyển qua nó mất mạng. Cần chế độ "thả": luồng không có luật thì trả về `false` ngay để hệ thống tự xử lý (có từ macOS 11), và tắt tính năng là gỡ cấu hình proxy ngay.
5. Xung đột với VPN của công ty và với các transparent proxy khác (Little Snitch, phần mềm an ninh). Giữ nguyên cách `core-netmerge` đang làm: khi VPN chở toàn bộ lưu lượng thì không chuyển gì, để không có gì đi vòng qua VPN.

Diễn đàn Apple có báo cáo dùng `requiredInterface` trong transparent proxy chạy được, nhưng kỹ sư Apple nói chính họ chưa thử. Cần một bản thử nhỏ trên macOS 13, 15 và 26 trước khi cam kết.

## Cách không cần tài khoản Apple Developer

Một tài khoản Apple miễn phí không cấp entitlement Network Extension, và profile của nó hết hạn sau 7 ngày. Tắt SIP thì chỉ hợp cho máy của người phát triển. Vì vậy, muốn điều khiển lưu lượng mà không trả phí cho Apple, phải đi đường mà các bản sing-box và Clash chạy độc lập trên macOS đang đi: TUN cùng một helper chạy bằng root.

1. Helper root: một daemon launchd nhỏ, cài một lần bằng mật khẩu quản trị (plist trong `/Library/LaunchDaemons`). Nó chỉ nhận lệnh từ Figghost qua một socket Unix có kiểm tra người gọi. Cần thử xem ký ad hoc dùng được `SMAppService.daemon` không; nếu không thì cài theo cách cũ là chép plist bằng quyền quản trị.
2. TUN: root mở được interface `utun` qua `com.apple.net.utun_control` mà không cần entitlement, như wireguard-go làm. Đặt route mặc định vào `utun` để mọi gói đi qua Figghost.
3. Stack TCP/IP trong user space dựng lại luồng từ các gói. Phía Rust có `smoltcp`, `ipstack`, `netstack-smoltcp`.
4. Biết luồng thuộc app nào: đọc bảng socket của kernel qua sysctl (các bản ghi `xinpcb_n`, `xsocket_n`), khớp cổng nguồn với PID, rồi từ PID ra đường dẫn app bằng `proc_pidpath`. sing-box làm đúng cách này cho luật `process_name` và `process_path` khi chạy độc lập trên macOS.
5. Ra ngoài: mở socket ghim vào `en0`, `en7`... bằng `IP_BOUND_IF`, đúng hàm `connect` của `core-netmerge`. Socket đã ghim thì không quay lại `utun`, nên không bị vòng lặp. Giới hạn tốc độ, ưu tiên và nút Tổng đều làm ở bước chuyển tiếp này, như phương án extension.

Cái giá của cách này:
- Một tiến trình root là bề mặt tấn công. Nó phải nhỏ, chỉ nhận một bộ lệnh cố định, và không bao giờ chạy lệnh shell từ tham số.
- DNS cần xử lý riêng: hoặc chuyển tiếp DNS qua đường đã chọn, hoặc dùng fake-ip như Clash.
- Xung đột với VPN khác, vì cùng tranh route mặc định. Khi phát hiện một VPN khác đang chạy, Figghost nhường và chỉ còn chế độ Theo dõi.
- CPU: stack user space tốn hơn kernel. Phải đo ở 500 Mbit/s đến 1 Gbit/s trên Apple silicon.
- Khi helper chết, phải tự gỡ route, nếu không cả máy mất mạng. launchd khởi động lại helper, và helper dọn route khi khởi động.

Một bậc nhẹ hơn, không cần TUN: đặt proxy SOCKS hoặc HTTP của hệ thống (`networksetup`, cần mật khẩu quản trị một lần) trỏ vào proxy của `core-netmerge`. App tôn trọng proxy hệ thống, gồm trình duyệt và phần lớn app dùng CFNetwork, sẽ đi qua Figghost. Từ cổng nguồn trên 127.0.0.1, Figghost tìm ra PID của app cùng người dùng mà không cần root, nên chọn đường, giới hạn và ưu tiên theo app được ngay. Game, công cụ dòng lệnh và app bỏ qua proxy thì không bị ảnh hưởng.

Đề xuất cho dự án mã nguồn mở: bậc proxy hệ thống làm trước, vì gần như là `core-netmerge` cộng thêm việc tra PID. Bậc TUN cùng helper root làm sau, như một tùy chọn "chế độ toàn máy". Phương án Network Extension chỉ giữ lại nếu sau này có tài khoản Developer.

## Thiết kế

### Canvas

- Popup Liquid Glass như Jarvis và Dictation, dùng chung `panel.rs`. Canvas vẽ trong web view của trang cài đặt (SVG) hoặc một cửa sổ riêng; SVG trong web view dễ làm kéo thả, zoom và cuộn hơn.
- Ba cột, nối bằng dây cong:
  1. Cột nguồn: mỗi đường mạng một khối (nhãn, interface, IP nội bộ, trạng thái lên hoặc xuống, tốc độ hiện tại, đồ thị nhỏ). Đường mới cắm vào (điện thoại USB) tự hiện, rút ra thì mờ đi.
  2. Cột gộp: mặc định có một nút "Tổng" nối với mọi đường đang lên, và mỗi đường cũng có thể nối thẳng sang app. Người dùng có thể tạo thêm nút tổng chỉ gồm vài đường.
  3. Cột app: các app đang có lưu lượng (từ `nettop`), biểu tượng app, tốc độ xuống và lên, tổng phiên. Một khối "Mọi app khác" luôn có, mặc định nối vào "Tổng" hoặc vào đường hệ thống chọn.
- Dây dày theo tốc độ đang chạy, có hạt chạy dọc dây khi có dữ liệu (tắt khi bật Reduce motion). Mỗi app có hai cổng nối: "chính" và "dự phòng". Dây chính là đường app dùng; dây dự phòng chỉ dùng khi đường chính rớt.
- Ưu tiên: mỗi app có một bậc (Cao, Bình thường, Thấp). "App ở dây sau không được ưu tiên như app chính" được hiểu là thứ tự trên dây: app nối sau vào cùng một nút nhận bậc thấp hơn app nối trước, và có thể kéo để đổi thứ tự. Đây là điểm cần chốt, xem mục Câu hỏi.
- Bấm vào một app hoặc một dây: bảng nhỏ với giới hạn xuống, giới hạn lên, bậc ưu tiên, nút "Gỡ luật".
- Có hai chế độ: Theo dõi (luôn có, không cần extension; dây chỉ phản ánh đường app đang thực sự đi) và Điều khiển (cần extension; kéo dây là đổi luật). Khi chưa cài extension, kéo dây hiện ghi chú nói rõ thiếu gì và nút cài.

### Theo dõi (phần làm trước)

- Chạy `nettop` ở chế độ ghi liên tục (`-L 0 -s 1`) chỉ khi canvas hoặc tab đang mở, đọc CSV từ stdout, gộp theo tiến trình và theo interface. Tiến trình con gom về app cha qua `proc_pidpath` và bundle của nó (Orca Helper về Orca).
- Lưu tổng theo ngày theo app (SQLite trong thư mục dữ liệu, giống Screen Time của figghost). Mục "Tổng kết": hôm nay, 7 ngày, app dùng nhiều nhất, chia theo đường.
- Có thể chuyển sang framework riêng tư `NetworkStatistics` sau nếu `nettop` tốn CPU; đo trước khi quyết.

### Điều khiển (phần cần extension)

- Lõi trong Rust, crate mới `core-netcontrol`: bảng luật (app theo bundle id hoặc signing id, đích là một đường hay một nút tổng, giới hạn, bậc ưu tiên), token bucket, hàng đợi có trọng số, và bộ lập lịch đem từ `core-netmerge` (tách phần chọn đường ra khỏi phần SOCKS để cả hai dùng chung).
- Extension: nhận luồng, tra luật theo app; không có luật thì trả về `false`. Có luật thì mở `NWConnection` với `requiredInterface`, chuyển tiếp hai chiều qua lõi Rust để giới hạn và đếm byte.
- App và extension trao đổi qua XPC hoặc qua `NETransparentProxyProviderProtocol` (cấu hình) cộng một kênh đếm. Luật được ghi vào `config.rs` như mọi trang khác, có mặc định và kẹp giá trị.
- UDP và QUIC: transparent proxy nhận cả UDP; QUIC chạy qua UDP nên vẫn ghim được đường. DNS để hệ thống tự xử lý.
- IPv6: `core-netmerge` hiện chỉ IPv4. Extension dùng `NWConnection` theo tên miền nên hệ thống tự chọn họ địa chỉ, ít vướng hơn.

### Chia mạng sang điện thoại (beta)

- Android: phát hiện thiết bị qua `adb devices`, nút "Chia mạng của Mac cho điện thoại này". Lần đầu cài APK relay qua `adb install`, sau đó `adb reverse` và khởi chạy dịch vụ VPN trên điện thoại. Trên canvas điện thoại là một khối ở cột app, nối từ một đường hoặc từ "Tổng" như mọi app khác. Phần relay phía Mac chạy trong tiến trình figghost, nên khi có extension thì lưu lượng của điện thoại cũng giới hạn và gộp được.
- iPhone: chỉ hướng dẫn và mở Settings, Sharing, Internet Sharing. Không hứa tự động.

## Trạng thái

Bản 2.2.10 đã làm phần Theo dõi, Nối dây qua proxy hệ thống (Bậc 1) và chia mạng cho Android qua gnirehtet. Mã nằm ở `crates/core-netcontrol` và `crates/app-ui/src/features/network`. Bậc TUN cùng helper root chưa làm.

## Thứ tự làm đề xuất

1. Theo dõi: `nettop`, canvas ở chế độ Theo dõi, tổng kết theo ngày. Không đổi gì cách phát hành, giá trị dùng ngay.
2. Điều khiển qua proxy hệ thống: luật nối dây theo app (tra PID từ cổng nguồn), nút Tổng dùng bộ lập lịch net-merge, giới hạn tốc độ, ưu tiên. Không cần tài khoản Apple.
3. Bản thử TUN cùng helper root: ghim một app không theo proxy (ví dụ một game) vào `en0`, đo CPU ở tốc độ cao, thử trên macOS 13, 15, 26. Đi tiếp hay dừng tùy kết quả. Phương án Network Extension chỉ xét lại nếu có tài khoản Developer.
4. Beta chia mạng sang Android qua USB.

Mọi phần tắt theo mặc định, tắt là gỡ cấu hình proxy, dừng `nettop` và không để gì chạy, như các tính năng khác.

## Câu hỏi cần chốt

1. Có chấp nhận một helper chạy bằng root (cài một lần bằng mật khẩu quản trị) cho chế độ toàn máy không. Không có helper thì chỉ điều khiển được những app theo proxy hệ thống.
2. "Dây sau" nghĩa là thứ tự nối vào cùng một nút, hay là app nối vào dây dự phòng. Ghi chép này tạm hiểu là thứ tự nối, và tách riêng khái niệm dây dự phòng.
3. Giới hạn tốc độ tính theo từng app hay theo từng nút (ví dụ cả nhóm app trên Wi-Fi không quá 20 Mbit/s).
4. Chia mạng sang điện thoại: chỉ Android là chấp nhận được cho beta không.

## Nguồn

- Apple, NETransparentProxyProvider: https://developer.apple.com/documentation/networkextension/netransparentproxyprovider
- Apple, App proxy provider: https://developer.apple.com/documentation/networkextension/app-proxy-provider
- Diễn đàn Apple về `requiredInterface` trong transparent proxy: https://developer.apple.com/forums/thread/738794
- gnirehtet (Genymobile): https://github.com/Genymobile/gnirehtet
- Chia mạng Mac sang iPhone qua USB: https://www.webnots.com/how-to-share-your-macs-internet-connection-with-iphone-or-ipad/
- Mac sang iPhone qua USB trên diễn đàn Apple: https://developer.apple.com/forums/thread/7334
- net-merge: https://github.com/net-merge/net-merge
- sing-box, luật theo tiến trình: https://sing-box.sagernet.org/configuration/route/rule
- sing-box, tính năng bản Apple (Network Extension so với bản độc lập): https://sing-box.sagernet.org/clients/apple/features/
- sing-box, cách tra tiến trình trên Darwin: https://singbox-internals.hidandelion.com/advanced/process.html
