// Colour picker, and background removal for images.
(window.__plugins = window.__plugins || []).push({
  id: 'vision',
  icon: '<path d="M14 6l4 4M15.5 4.5a2.1 2.1 0 0 1 4 .3 2.1 2.1 0 0 1-.6 1.5L17 8l-2.5-2.5zM13.5 7.5l3 3-8.4 8.4a2 2 0 0 1-1.4.6H4.5v-2.2a2 2 0 0 1 .6-1.4z"/>',
  en: {
    'nav.vision': 'Colour picker', 'page.vision': 'Colour picker',
    'page.vision.sub': 'Point at any pixel on the screen, see it magnified and copy its colour.',
    'target.vision': 'the colour picker shortcut',
    'vision.enable.title': 'Colour picker', 'vision.enable.desc': 'A shortcut opens a loupe at the pointer. A click copies the colour, Tab changes the format and Esc cancels.',
    'sec.vision.keys': 'Shortcut and format',
    'vision.shortcut.title': 'Shortcut', 'vision.shortcut.desc': 'Starts the picker from any app.', 'vision.shortcut.clear': 'Remove shortcut', 'vision.shortcut.none': 'None',
    'vision.format.title': 'Format', 'vision.format.desc': 'The format a pick starts in. Press Tab while picking to try the others.',
    'sec.vision.access': 'Screen Recording',
    'vision.access.title': 'Screen Recording access', 'vision.access.desc': 'Reading the pixels of other apps needs this permission. After you allow it, macOS may ask to quit and reopen Figghost.',
    'vision.access.granted': 'Allowed', 'vision.access.missing': 'Not allowed', 'vision.access.allow': 'Allow',
    'sec.vision.more': 'Images',
    'vision.bg.title': 'Remove background', 'vision.bg.desc': 'Right-click an image on the shelf and choose Remove background. A new PNG with a transparent background is made next to the original. Needs macOS 14 or later.',
    'vision.bg.yes': 'Available', 'vision.bg.no': 'Needs macOS 14',
    'vision.text.title': 'Text recognition', 'vision.text.desc': 'Reads the text in an image, English and Vietnamese included, for the screen capture tools. Everything is done on this Mac.',
    'vision.text.yes': 'Available', 'vision.text.no': 'Not available',
  },
  vi: {
    'nav.vision': 'Chọn màu', 'page.vision': 'Chọn màu',
    'page.vision.sub': 'Trỏ vào bất kỳ điểm ảnh nào trên màn hình, xem phóng to và chép mã màu.',
    'target.vision': 'phím tắt chọn màu',
    'vision.enable.title': 'Chọn màu', 'vision.enable.desc': 'Một phím tắt mở kính lúp tại con trỏ. Nhấp để chép màu, Tab đổi dạng mã màu và Esc để hủy.',
    'sec.vision.keys': 'Phím tắt và dạng mã màu',
    'vision.shortcut.title': 'Phím tắt', 'vision.shortcut.desc': 'Bắt đầu chọn màu từ bất kỳ ứng dụng nào.', 'vision.shortcut.clear': 'Xóa phím tắt', 'vision.shortcut.none': 'Không có',
    'vision.format.title': 'Dạng mã màu', 'vision.format.desc': 'Dạng mã màu khi bắt đầu chọn. Nhấn Tab khi đang chọn để thử các dạng khác.',
    'sec.vision.access': 'Ghi màn hình',
    'vision.access.title': 'Quyền Ghi màn hình', 'vision.access.desc': 'Đọc điểm ảnh của ứng dụng khác cần quyền này. Sau khi bạn cho phép, macOS có thể yêu cầu thoát và mở lại Figghost.',
    'vision.access.granted': 'Đã cho phép', 'vision.access.missing': 'Chưa cho phép', 'vision.access.allow': 'Cho phép',
    'sec.vision.more': 'Hình ảnh',
    'vision.bg.title': 'Xóa nền', 'vision.bg.desc': 'Bấm chuột phải vào một ảnh trên khay tệp và chọn Xóa nền. Một tệp PNG mới có nền trong suốt được tạo cạnh tệp gốc. Cần macOS 14 trở lên.',
    'vision.bg.yes': 'Có sẵn', 'vision.bg.no': 'Cần macOS 14',
    'vision.text.title': 'Nhận dạng chữ', 'vision.text.desc': 'Đọc chữ trong ảnh, gồm cả tiếng Anh và tiếng Việt, cho các công cụ chụp màn hình. Mọi thứ được xử lý ngay trên máy này.',
    'vision.text.yes': 'Có sẵn', 'vision.text.no': 'Không có',
  },
  build() {
    const clear = h('button', {
      class: 'btn icon', 'aria-label': t('vision.shortcut.clear'), title: t('vision.shortcut.clear'),
      onclick: () => send({ t: 'hotkey', target: 'vision', value: '' }),
    }, icon('close'));
    reg(() => { clear.style.visibility = S.vision.shortcut ? 'visible' : 'hidden'; });
    const formats = ['hex', 'rgb', 'hsl', 'hsb'];
    const when = { when: () => S.vision.enabled };
    return [
      card(toggleRow('vision.enabled', t('vision.enable.title'), t('vision.enable.desc'))),
      sectionTitle(t('sec.vision.keys')),
      card(
        row(t('vision.shortcut.title'), t('vision.shortcut.desc'),
          h('div', { class: 'vision-controls' }, hotkeyButton('vision', { allowClear: true, label: t('vision.shortcut.title') }), clear), when),
        row(t('vision.format.title'), t('vision.format.desc'), segmented({
          label: t('vision.format.title'),
          options: formats.map((f) => ({ value: f, label: f.toUpperCase() })),
          value: () => S.vision.format,
          onPick: (f) => change('vision.format', f),
        }), when)),
      sectionTitle(t('sec.vision.access')),
      card(row(t('vision.access.title'), t('vision.access.desc'), [
        pill(() => (S.vision.screen_access ? t('vision.access.granted') : t('vision.access.missing')), () => S.vision.screen_access),
        button(t('vision.access.allow'), () => send({ t: 'action', name: 'vision_screen_access' }), { small: true }),
      ])),
      sectionTitle(t('sec.vision.more')),
      card(
        row(t('vision.bg.title'), t('vision.bg.desc'),
          pill(() => (S.vision.background_removal ? t('vision.bg.yes') : t('vision.bg.no')), () => S.vision.background_removal)),
        row(t('vision.text.title'), t('vision.text.desc'),
          pill(() => (S.vision.text_recognition ? t('vision.text.yes') : t('vision.text.no')), () => S.vision.text_recognition))),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('vision');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const power = page.querySelector('.switch');
    const was = S.vision.enabled;
    power.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'vision.enabled' && m.value === !was), 'vision: the switch sends its setting: ' + JSON.stringify(sent));
    power.click();

    // The shortcut recorder sends the target of this page, and the clear button removes it.
    sent.length = 0;
    const hot = page.querySelector('.hotkey');
    hot.click();
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'Digit9', metaKey: true, ctrlKey: true, bubbles: true, cancelable: true }));
    expect(sent.some((m) => m.t === 'hotkey' && m.target === 'vision' && m.value === 'ctrl+cmd+9'), 'vision: a shortcut is sent: ' + JSON.stringify(sent));
    sent.length = 0;
    const clear = page.querySelector('.vision-controls .btn.icon');
    expect(clear.style.visibility === 'visible', 'vision: a shortcut that is set can be removed');
    clear.click();
    expect(sent.some((m) => m.t === 'hotkey' && m.target === 'vision' && m.value === ''), 'vision: removing sends an empty shortcut');
    S.vision.shortcut = '';
    go('vision');
    expect(page.querySelector('.vision-controls .btn.icon').style.visibility === 'hidden', 'vision: nothing to remove without a shortcut');

    // The format picker writes the format.
    sent.length = 0;
    const radios = [...page.querySelectorAll('.seg button')];
    expect(radios.length === 4, 'vision: four formats');
    radios[2].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'vision.format' && m.value === 'hsl'), 'vision: the format is sent: ' + JSON.stringify(sent));
    expect(radios[2].getAttribute('aria-checked') === 'true', 'vision: the chosen format is marked');

    // The permission button asks the app.
    sent.length = 0;
    [...page.querySelectorAll('.btn')].find((b) => b.textContent === 'Allow').click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'vision_screen_access'), 'vision: the permission button sends its action');

    // What the Mac can do shows as state, not as a failure.
    S.vision.background_removal = false;
    S.vision.screen_access = true;
    go('vision');
    expect(page.textContent.includes('Needs macOS 14') && page.textContent.includes('Allowed'), 'vision: the state of the Mac is shown');
  },
});
