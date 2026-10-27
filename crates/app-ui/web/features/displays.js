// Displays: a tab of the notch with the brightness and contrast of the external displays, the colour profile of
// every display, the arrangement of the displays when there is more than one, and the brightness keys for the
// external ones.
(window.__plugins = window.__plugins || []).push({
  id: 'displays',
  icon: 'sun.max.fill',
  en: {
    'nav.displays': 'Displays', 'page.displays': 'Displays',
    'page.displays.sub': 'Set the brightness and contrast of an external display from a tab of the notch, as its own buttons would, the color profile of any display, and where the displays are.',
    'displays.enable.title': 'Show the Displays tab', 'displays.enable.desc': 'Each external display that answers over DDC/CI gets a card with a slider for its brightness and one for its contrast, and the display itself changes, not a filter over the picture. The displays are looked for when the tab opens. It needs no permission. A display behind a hub, a DisplayLink adapter or some HDMI ports does not answer.',
    'sec.displays.profiles': 'Color profiles',
    'displays.profiles.title': 'Choose the color profile of each display', 'displays.profiles.desc': 'Every display gets a card, the built-in one too, with a menu of its own profile and the ones macOS comes with: Display P3, sRGB, Adobe RGB, Rec. 709, Rec. 2020 and the others System Settings lists under Displays. The choice is the same as there, for this user, and the picture changes at once; choosing the display\'s own profile puts it back.',
    'sec.displays.arrange': 'Arrangement',
    'displays.arrange.title': 'Arrange the displays from the tab', 'displays.arrange.desc': 'With more than one display connected, the tab has a second view, Arrange, with the displays drawn where they are, as in System Settings. Drag a display to move it and it goes flush against the nearest side of another; drag the white bar onto another display to make it the main one, with the menu bar and the Dock. Its menu chooses what the display is used as: the main display, an extended display, or a mirror of another. The change is kept, as System Settings keeps it.',
    'sec.displays.keys': 'Keys',
    'displays.keys.title': 'Brightness keys set the external displays', 'displays.keys.desc': 'The brightness keys move every external display together and show the level in the look of the volume indicator; Shift and Option together take a smaller step. While a built-in screen is lit the keys stay with it, and Control with a key sets the external displays. Needs Accessibility.',
  },
  vi: {
    'nav.displays': 'Màn hình', 'page.displays': 'Màn hình',
    'page.displays.sub': 'Chỉnh độ sáng và độ tương phản của màn hình rời ngay trong một tab của notch, như bấm nút trên màn hình, chọn cấu hình màu cho mọi màn hình và sắp xếp vị trí các màn hình.',
    'displays.enable.title': 'Hiện tab Màn hình', 'displays.enable.desc': 'Mỗi màn hình rời trả lời qua DDC/CI có một thẻ với thanh trượt độ sáng và độ tương phản, và chính màn hình đổi theo, không phải một lớp phủ lên hình. Máy chỉ tìm màn hình khi tab mở. Không cần cấp quyền. Màn hình cắm qua hub, bộ chuyển DisplayLink hoặc một số cổng HDMI sẽ không trả lời.',
    'sec.displays.profiles': 'Cấu hình màu',
    'displays.profiles.title': 'Chọn cấu hình màu cho từng màn hình', 'displays.profiles.desc': 'Mọi màn hình đều có một thẻ, kể cả màn hình liền máy, với menu gồm cấu hình riêng của màn hình và các cấu hình có sẵn của macOS: Display P3, sRGB, Adobe RGB, Rec. 709, Rec. 2020 và các cấu hình khác mà Cài đặt hệ thống liệt kê ở mục Màn hình. Lựa chọn giống hệt trong đó, áp dụng cho người dùng này, và hình đổi ngay; chọn lại cấu hình riêng của màn hình để trở về như cũ.',
    'sec.displays.arrange': 'Sắp xếp',
    'displays.arrange.title': 'Sắp xếp màn hình ngay trong tab', 'displays.arrange.desc': 'Khi cắm từ hai màn hình trở lên, tab có thêm chế độ Sắp xếp, vẽ các màn hình đúng vị trí như trong Cài đặt hệ thống. Kéo một màn hình để đổi chỗ, nó sẽ áp sát cạnh gần nhất của màn hình khác; kéo thanh trắng sang màn hình khác để đặt màn hình đó làm màn hình chính, có thanh menu và Dock. Menu của từng màn hình chọn cách dùng: màn hình chính, màn hình mở rộng, hoặc phản chiếu một màn hình khác. Thay đổi được giữ lại như khi chỉnh trong Cài đặt hệ thống.',
    'sec.displays.keys': 'Phím',
    'displays.keys.title': 'Phím độ sáng chỉnh màn hình rời', 'displays.keys.desc': 'Phím độ sáng chỉnh mọi màn hình rời cùng lúc và hiện mức theo kiểu của bảng âm lượng; giữ Shift và Option để bước nhỏ hơn. Khi màn hình liền máy đang sáng, phím vẫn dành cho nó, giữ Control khi bấm để chỉnh màn hình rời. Cần quyền Trợ năng.',
  },
  build() {
    const on = () => S.displays.enabled;
    return [
      card(toggleRow('displays.enabled', t('displays.enable.title'), t('displays.enable.desc'))),
      sectionTitle(t('sec.displays.profiles')),
      card(toggleRow('displays.profiles', t('displays.profiles.title'), t('displays.profiles.desc'), { when: on })),
      sectionTitle(t('sec.displays.arrange')),
      card(toggleRow('displays.arrange', t('displays.arrange.title'), t('displays.arrange.desc'), { when: on })),
      sectionTitle(t('sec.displays.keys')),
      card(toggleRow('displays.keys', t('displays.keys.title'), t('displays.keys.desc'), { when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('displays');
    const page = document.querySelector('#scroller');
    const switches = [...page.querySelectorAll('.switch')];
    expect(switches.length === 4, 'displays: a switch for the tab, the profiles, the arrangement and the keys');
    sent.length = 0;
    const was = S.displays.enabled;
    switches[0].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'displays.enabled' && m.value === !was), 'displays: the switch sends its setting: ' + JSON.stringify(sent));
    switches[0].click();
    sent.length = 0;
    switches[1].click();
    expect(sent.some((m) => m.path === 'displays.profiles'), 'displays: the profiles switch sends its own path: ' + JSON.stringify(sent));
    switches[1].click();
    sent.length = 0;
    switches[2].click();
    expect(sent.some((m) => m.path === 'displays.arrange'), 'displays: the arrangement switch sends its own path: ' + JSON.stringify(sent));
    switches[2].click();
    sent.length = 0;
    switches[3].click();
    expect(sent.some((m) => m.path === 'displays.keys'), 'displays: the keys switch sends its own path: ' + JSON.stringify(sent));
    switches[3].click();
  },
});
