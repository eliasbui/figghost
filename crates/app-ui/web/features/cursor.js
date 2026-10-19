// Custom cursor: the arrow and the hand over links become a rainbow arrow, a pixel character or a picture of your own.
(window.__plugins = window.__plugins || []).push({
  id: 'cursor',
  icon: 'cursorarrow.rays',
  en: {
    'nav.cursor': 'Cursor', 'page.cursor': 'Cursor',
    'page.cursor.sub': 'Give the pointer a look of its own: a rainbow arrow, a pixel character that blinks, or any picture or GIF you choose.',
    'cursor.enable.title': 'Custom cursor',
    'cursor.enable.desc': 'The arrow and the hand over links are drawn from the picture you choose below, and the system animates them, so nothing runs while the pointer moves. The text cursor stays as it is. The pointer goes back to normal when you turn this off or quit Figghost.',
    'sec.cursor.look': 'Look',
    'cursor.theme.title': 'Picture', 'cursor.theme.desc': 'A character stands and blinks, and beams when you point at a link.',
    'cursor.theme.neon': 'Rainbow arrow', 'cursor.theme.spider': 'Spider', 'cursor.theme.frog': 'Frog', 'cursor.theme.ufo': 'UFO',
    'cursor.theme.ghost': 'Ghost', 'cursor.theme.wizard': 'Wizard', 'cursor.theme.mario': 'Mario', 'cursor.theme.custom': 'Your picture',
    'cursor.custom.title': 'Your own picture', 'cursor.custom.desc': 'A GIF or an animated PNG plays as it is; any other picture stays still. Transparent backgrounds work.',
    'cursor.custom.choose': 'Choose picture', 'cursor.custom.none': 'No picture chosen',
    'cursor.custom.bad': 'This picture cannot be read. Choose another one.',
    'cursor.tip.title': 'Point that clicks', 'cursor.tip.desc': 'Where on your picture the click lands.',
    'cursor.tip.top': 'Top left', 'cursor.tip.centre': 'Centre',
    'sec.cursor.feel': 'Size and speed',
    'cursor.size.title': 'Size', 'cursor.size.desc': 'How wide the pointer is, in points. The system arrow is about 17.',
    'cursor.speed.title': 'Speed', 'cursor.speed.desc': 'How fast the animation plays.',
    'cursor.note.title': 'Where it shows', 'cursor.note.desc': 'Over every other app. This window keeps the system pointer while it is in front, which is how macOS lets a program draw its own cursor, so look at another window to see the change.',
  },
  vi: {
    'nav.cursor': 'Con trỏ', 'page.cursor': 'Con trỏ',
    'page.cursor.sub': 'Cho con trỏ chuột một diện mạo riêng: mũi tên cầu vồng, nhân vật pixel biết chớp mắt, hoặc bất kỳ ảnh hay GIF nào bạn chọn.',
    'cursor.enable.title': 'Con trỏ tùy chỉnh',
    'cursor.enable.desc': 'Mũi tên và bàn tay khi trỏ vào liên kết được vẽ từ hình bạn chọn bên dưới, hệ thống tự chạy hoạt ảnh nên không có gì chạy khi con trỏ di chuyển. Con trỏ văn bản giữ nguyên. Con trỏ trở lại như cũ khi bạn tắt tính năng này hoặc thoát Figghost.',
    'sec.cursor.look': 'Diện mạo',
    'cursor.theme.title': 'Hình', 'cursor.theme.desc': 'Nhân vật đứng yên và chớp mắt, rồi hớn hở khi bạn trỏ vào liên kết.',
    'cursor.theme.neon': 'Mũi tên cầu vồng', 'cursor.theme.spider': 'Nhện', 'cursor.theme.frog': 'Ếch', 'cursor.theme.ufo': 'UFO',
    'cursor.theme.ghost': 'Ma', 'cursor.theme.wizard': 'Phù thủy', 'cursor.theme.mario': 'Mario', 'cursor.theme.custom': 'Hình của bạn',
    'cursor.custom.title': 'Hình của riêng bạn', 'cursor.custom.desc': 'GIF hoặc PNG động sẽ chạy đúng như vậy; ảnh khác đứng yên. Nền trong suốt dùng được.',
    'cursor.custom.choose': 'Chọn hình', 'cursor.custom.none': 'Chưa chọn hình',
    'cursor.custom.bad': 'Không đọc được hình này. Hãy chọn hình khác.',
    'cursor.tip.title': 'Điểm nhấp chuột', 'cursor.tip.desc': 'Vị trí trên hình nơi cú nhấp được tính.',
    'cursor.tip.top': 'Góc trên trái', 'cursor.tip.centre': 'Giữa',
    'sec.cursor.feel': 'Kích thước và tốc độ',
    'cursor.size.title': 'Kích thước', 'cursor.size.desc': 'Chiều rộng của con trỏ, tính bằng point. Mũi tên của hệ thống khoảng 17.',
    'cursor.speed.title': 'Tốc độ', 'cursor.speed.desc': 'Tốc độ chạy của hoạt ảnh.',
    'cursor.note.title': 'Hiển thị ở đâu', 'cursor.note.desc': 'Trên mọi ứng dụng khác. Cửa sổ này giữ con trỏ của hệ thống khi nó ở phía trước, vì macOS chỉ cho chương trình tự vẽ con trỏ theo cách đó, nên hãy nhìn sang cửa sổ khác để thấy thay đổi.',
  },
  build() {
    const art = (theme) => {
      if (theme === 'neon') return h('div', { class: 'cursor-art' }, h('div', { class: 'cursor-neon-frame' }, h('div', { class: 'cursor-neon' })));
      if (theme === 'custom') return h('div', { class: 'cursor-art' }, icon('folder'));
      const sprite = h('div', { class: 'cursor-sprite', role: 'img' });
      reg(() => { sprite.style.backgroundImage = `url("${(S.cursor.sheets || {})[theme] || ''}")`; });
      return h('div', { class: 'cursor-art' }, sprite);
    };
    const choose = () => send({ t: 'action', name: 'cursor_choose' });
    const tiles = h('div', { class: 'cursor-tiles', role: 'radiogroup', 'aria-label': t('cursor.theme.title') },
      S.cursor.themes.map((theme) => {
        const label = h('span', { class: 'cursor-label', text: t('cursor.theme.' + theme) });
        const el = h('button', {
          class: 'cursor-tile', role: 'radio', 'aria-checked': 'false', 'data-theme': theme,
          // Until a picture has been chosen, the tile of your own picture asks for one.
          onclick: () => (theme === 'custom' && !S.cursor.custom_name ? choose() : change('cursor.theme', theme)),
        }, art(theme), label);
        reg(() => {
          el.setAttribute('aria-checked', String(S.cursor.theme === theme));
          label.textContent = t('cursor.theme.' + theme);
        });
        return el;
      }));
    const file = h('span', { class: 'cursor-file' });
    reg(() => { file.textContent = S.cursor.custom_name || t('cursor.custom.none'); });
    const bad = pill(() => t('cursor.custom.bad'), () => false);
    reg(() => { bad.style.display = S.cursor.unreadable ? '' : 'none'; });
    const customControl = h('div', { class: 'row-control' }, bad, file, button(t('cursor.custom.choose'), choose, { small: true }));
    const on = () => S.cursor.enabled;
    return [
      card(toggleRow('cursor.enabled', t('cursor.enable.title'), t('cursor.enable.desc'))),
      sectionTitle(t('sec.cursor.look')),
      card(
        row(t('cursor.theme.title'), t('cursor.theme.desc'), tiles, { block: true, when: on }),
        row(t('cursor.custom.title'), t('cursor.custom.desc'), customControl, { when: on }),
        segRow('cursor.tip', t('cursor.tip.title'), t('cursor.tip.desc'), ['top', 'centre'], { when: () => S.cursor.enabled && S.cursor.theme === 'custom' })),
      sectionTitle(t('sec.cursor.feel')),
      card(
        sliderRow({ path: 'cursor.size', title: t('cursor.size.title'), desc: t('cursor.size.desc'), min: 16, max: 96, step: 2, format: (v) => Math.round(v) + ' pt' }),
        sliderRow({ path: 'cursor.speed', title: t('cursor.speed.title'), desc: t('cursor.speed.desc'), min: 0.25, max: 3, step: 0.05, format: (v) => (+v).toFixed(2).replace(/0$/, '') + 'x' })),
      card(infoRow(t('cursor.note.title'), t('cursor.note.desc'))),
    ];
  },
  selftest({ expect, go, sent, S }) {
    for (const code of ['en', 'vi']) {
      setLang(code);
      go('cursor');
      const tiles = [...document.querySelectorAll('.cursor-tile')];
      expect(tiles.length === S.cursor.themes.length, `${code}: a tile for every picture`);
      for (const theme of S.cursor.themes) expect(t('cursor.theme.' + theme) !== 'cursor.theme.' + theme, `${code}: ${theme} has a name`);
      expect(tiles.filter((b) => b.getAttribute('aria-checked') === 'true').length === 1, `${code}: exactly one picture is chosen`);
    }
    setLang('en');
    go('cursor');
    const first = document.querySelector('.switch');
    const was = S.cursor.enabled;
    sent.length = 0;
    first.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'cursor.enabled' && m.value === !was), 'the switch sends its setting: ' + JSON.stringify(sent));
    first.click();
    sent.length = 0;
    document.querySelector('.cursor-tile[data-theme="spider"]').click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'cursor.theme' && m.value === 'spider'), 'a picture is chosen: ' + JSON.stringify(sent));
    sent.length = 0;
    const had = S.cursor.custom_name;
    S.cursor.custom_name = '';
    document.querySelector('.cursor-tile[data-theme="custom"]').click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'cursor_choose'), 'your own picture asks for a file first: ' + JSON.stringify(sent));
    S.cursor.custom_name = had;
    sent.length = 0;
    const choose = [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === t('cursor.custom.choose'));
    expect(!!choose, 'a choose button');
    choose.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'cursor_choose'), 'the button asks for a file');
    sent.length = 0;
    const size = document.querySelector('input[type="range"]');
    size.value = 48;
    size.dispatchEvent(new Event('input'));
    expect(S.cursor.size === 48, 'the size follows the slider');
  },
});
