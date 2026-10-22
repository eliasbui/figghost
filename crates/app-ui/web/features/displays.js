// Displays: a tab of the notch with the brightness and contrast of the external displays, and the brightness keys
// for them.
(window.__plugins = window.__plugins || []).push({
  id: 'displays',
  icon: 'display',
  en: {
    'nav.displays': 'Displays', 'page.displays': 'Displays',
    'page.displays.sub': 'Set the brightness and contrast of an external display from a tab of the notch, as its own buttons would.',
    'displays.enable.title': 'Show the Displays tab', 'displays.enable.desc': 'Each external display that answers over DDC/CI gets a card with a slider for its brightness and one for its contrast, and the display itself changes, not a filter over the picture. The displays are looked for when the tab opens. It needs no permission. A display behind a hub, a DisplayLink adapter or some HDMI ports does not answer.',
    'sec.displays.keys': 'Keys',
    'displays.keys.title': 'Brightness keys set the external displays', 'displays.keys.desc': 'The brightness keys move every external display together and show the level in the look of the volume indicator; Shift and Option together take a smaller step. While a built-in screen is lit the keys stay with it, and Control with a key sets the external displays. Needs Accessibility.',
  },
  vi: {
    'nav.displays': 'Màn hình', 'page.displays': 'Màn hình',
    'page.displays.sub': 'Chỉnh độ sáng và độ tương phản của màn hình rời ngay trong một tab của notch, như bấm nút trên màn hình.',
    'displays.enable.title': 'Hiện tab Màn hình', 'displays.enable.desc': 'Mỗi màn hình rời trả lời qua DDC/CI có một thẻ với thanh trượt độ sáng và độ tương phản, và chính màn hình đổi theo, không phải một lớp phủ lên hình. Máy chỉ tìm màn hình khi tab mở. Không cần cấp quyền. Màn hình cắm qua hub, bộ chuyển DisplayLink hoặc một số cổng HDMI sẽ không trả lời.',
    'sec.displays.keys': 'Phím',
    'displays.keys.title': 'Phím độ sáng chỉnh màn hình rời', 'displays.keys.desc': 'Phím độ sáng chỉnh mọi màn hình rời cùng lúc và hiện mức theo kiểu của bảng âm lượng; giữ Shift và Option để bước nhỏ hơn. Khi màn hình liền máy đang sáng, phím vẫn dành cho nó, giữ Control khi bấm để chỉnh màn hình rời. Cần quyền Trợ năng.',
  },
  build() {
    const on = () => S.displays.enabled;
    return [
      card(toggleRow('displays.enabled', t('displays.enable.title'), t('displays.enable.desc'))),
      sectionTitle(t('sec.displays.keys')),
      card(toggleRow('displays.keys', t('displays.keys.title'), t('displays.keys.desc'), { when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('displays');
    const page = document.querySelector('#scroller');
    const switches = [...page.querySelectorAll('.switch')];
    expect(switches.length === 2, 'displays: a switch for the tab and one for the keys');
    sent.length = 0;
    const was = S.displays.enabled;
    switches[0].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'displays.enabled' && m.value === !was), 'displays: the switch sends its setting: ' + JSON.stringify(sent));
    switches[0].click();
    sent.length = 0;
    switches[1].click();
    expect(sent.some((m) => m.path === 'displays.keys'), 'displays: the keys switch sends its own path: ' + JSON.stringify(sent));
    switches[1].click();
  },
});
