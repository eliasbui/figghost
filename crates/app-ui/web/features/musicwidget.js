// Music widget: a player in Liquid Glass that floats on the desktop, in a few styles, anywhere on the screen.
(window.__plugins = window.__plugins || []).push({
  id: 'musicwidget',
  icon: 'music.note',
  en: {
    'nav.musicwidget': 'Music widget', 'page.musicwidget': 'Music widget',
    'page.musicwidget.sub': 'A player of Liquid Glass that floats on your desktop with the cover, the song and the controls of whatever plays. Drag it where you like and pull its corner to resize it.',
    'musicwidget.enable.title': 'Show the music widget',
    'musicwidget.enable.desc': 'Works with any player and with music in a browser. macOS lets no app draw on the lock screen, so the widget floats on every desktop instead, and over full-screen apps if you choose that below.',
    'sec.musicwidget.style': 'Style',
    'musicwidget.style.card': 'Card', 'musicwidget.style.cover': 'Cover', 'musicwidget.style.bar': 'Capsule', 'musicwidget.style.vinyl': 'Vinyl', 'musicwidget.style.minimal': 'Minimal',
    'musicwidget.style.card.desc': 'The cover, the song, a bar and the buttons.', 'musicwidget.style.cover.desc': 'A big cover with the song over its foot.', 'musicwidget.style.bar.desc': 'One slim row.', 'musicwidget.style.vinyl.desc': 'A record that turns while it plays.', 'musicwidget.style.minimal.desc': 'Only the song and one button.',
    'sec.musicwidget.glass': 'Glass',
    'musicwidget.material.title': 'Material', 'musicwidget.material.desc': 'How much of the desktop shows through the glass.',
    'musicwidget.material.glass': 'Liquid Glass', 'musicwidget.material.clear': 'Clear glass', 'musicwidget.material.dark': 'Dark glass', 'musicwidget.material.accent': 'Accent glass',
    'musicwidget.ambient.title': 'Light from the cover', 'musicwidget.ambient.desc': 'A soft blur of the cover glows behind the controls and takes the colours of the music.',
    'musicwidget.opacity.title': 'Opacity', 'musicwidget.opacity.desc': 'How solid the whole widget is.',
    'sec.musicwidget.place': 'Place and size',
    'musicwidget.layer.title': 'Stays', 'musicwidget.layer.desc': 'Where it floats, in front of or behind your windows.',
    'musicwidget.layer.desktop': 'On the desktop, under windows', 'musicwidget.layer.top': 'Above windows', 'musicwidget.layer.always': 'Above everything, full screen too', 'musicwidget.layer.lockscreen': 'On the lock screen',
    'musicwidget.hide_idle.title': 'Hide when nothing plays', 'musicwidget.hide_idle.desc': 'The widget goes away until music starts. Off keeps it on screen with a note.',
    'musicwidget.width.title': 'Width', 'musicwidget.width.desc': 'You can also pull the corner of the widget.',
    'musicwidget.height.title': 'Height', 'musicwidget.height.desc': 'The slim styles do not need much.',
    'musicwidget.reset.title': 'Back to the start', 'musicwidget.reset.desc': 'Puts it at the foot of the screen at the size of its style.', 'musicwidget.reset': 'Reset place and size',
    'musicwidget.px': '{n} pt', 'musicwidget.auto': 'Style default',
  },
  vi: {
    'nav.musicwidget': 'Widget nhạc', 'page.musicwidget': 'Widget nhạc',
    'page.musicwidget.sub': 'Trình phát bằng Liquid Glass nổi trên màn hình, hiện ảnh bìa, tên bài và các nút điều khiển của bất kỳ thứ gì đang phát. Kéo đến chỗ bạn thích và kéo góc để đổi kích thước.',
    'musicwidget.enable.title': 'Hiện widget nhạc',
    'musicwidget.enable.desc': 'Dùng được với mọi trình phát và nhạc trong trình duyệt. macOS không cho ứng dụng nào vẽ lên màn hình khóa, nên widget nổi trên mọi màn hình nền, và cả trên ứng dụng toàn màn hình nếu bạn chọn bên dưới.',
    'sec.musicwidget.style': 'Kiểu',
    'musicwidget.style.card': 'Thẻ', 'musicwidget.style.cover': 'Ảnh bìa', 'musicwidget.style.bar': 'Viên thuốc', 'musicwidget.style.vinyl': 'Đĩa than', 'musicwidget.style.minimal': 'Tối giản',
    'musicwidget.style.card.desc': 'Ảnh bìa, tên bài, thanh tiến độ và các nút.', 'musicwidget.style.cover.desc': 'Ảnh bìa lớn, tên bài nằm dưới chân.', 'musicwidget.style.bar.desc': 'Một hàng mảnh.', 'musicwidget.style.vinyl.desc': 'Đĩa quay khi đang phát.', 'musicwidget.style.minimal.desc': 'Chỉ tên bài và một nút.',
    'sec.musicwidget.glass': 'Kính',
    'musicwidget.material.title': 'Chất liệu', 'musicwidget.material.desc': 'Màn hình nền lộ ra qua kính nhiều hay ít.',
    'musicwidget.material.glass': 'Liquid Glass', 'musicwidget.material.clear': 'Kính trong', 'musicwidget.material.dark': 'Kính tối', 'musicwidget.material.accent': 'Kính màu nhấn',
    'musicwidget.ambient.title': 'Ánh sáng từ ảnh bìa', 'musicwidget.ambient.desc': 'Ảnh bìa mờ nhẹ tỏa sáng sau các nút và lấy màu của bài nhạc.',
    'musicwidget.opacity.title': 'Độ đậm', 'musicwidget.opacity.desc': 'Widget đặc đến đâu.',
    'sec.musicwidget.place': 'Vị trí và kích thước',
    'musicwidget.layer.title': 'Nằm ở', 'musicwidget.layer.desc': 'Nổi phía trước hay phía sau các cửa sổ của bạn.',
    'musicwidget.layer.desktop': 'Trên màn hình nền, dưới cửa sổ', 'musicwidget.layer.top': 'Trên cửa sổ', 'musicwidget.layer.always': 'Trên mọi thứ, cả toàn màn hình', 'musicwidget.layer.lockscreen': 'Trên màn hình khóa',
    'musicwidget.hide_idle.title': 'Ẩn khi không phát gì', 'musicwidget.hide_idle.desc': 'Widget biến mất cho đến khi nhạc phát. Tắt thì giữ nó trên màn hình kèm một dòng ghi chú.',
    'musicwidget.width.title': 'Chiều rộng', 'musicwidget.width.desc': 'Bạn cũng có thể kéo góc của widget.',
    'musicwidget.height.title': 'Chiều cao', 'musicwidget.height.desc': 'Các kiểu mảnh không cần cao.',
    'musicwidget.reset.title': 'Về như ban đầu', 'musicwidget.reset.desc': 'Đặt lại ở chân màn hình với kích thước của kiểu đang chọn.', 'musicwidget.reset': 'Đặt lại vị trí và kích thước',
    'musicwidget.px': '{n} pt', 'musicwidget.auto': 'Mặc định của kiểu',
  },
  build() {
    const on = () => S.musicwidget.enabled;
    const styles = ['card', 'cover', 'bar', 'vinyl', 'minimal'];
    const cards = h('div', { class: 'style-cards mw-cards', role: 'group', 'aria-label': t('sec.musicwidget.style') });
    for (const id of styles) {
      const name = h('span', { class: 'sc-name' });
      const desc = h('span', { class: 'sc-desc' });
      const el = h('button', { class: 'style-card', type: 'button', 'aria-pressed': 'false', onclick: () => change('musicwidget.style', id) },
        h('span', { class: 'sc-art mw-art mw-' + id, 'aria-hidden': 'true' }, h('i', { class: 'mw-cover' }), h('i', { class: 'mw-l1' }), h('i', { class: 'mw-l2' }), h('i', { class: 'mw-bar' })), name, desc);
      reg(() => {
        el.setAttribute('aria-pressed', String(S.musicwidget.style === id));
        name.textContent = t('musicwidget.style.' + id);
        desc.textContent = t('musicwidget.style.' + id + '.desc');
      });
      cards.append(el);
    }
    const wrap = h('div', { class: 'style-wrap' }, cards);
    reg(() => wrap.classList.toggle('disabled', !on()));
    const size = (path, key, min, max) => sliderRow({
      path, title: t(key + '.title'), desc: t(key + '.desc'), min, max, step: 10,
      format: (v) => (S.musicwidget[path.split('.')[1]] === 0 ? t('musicwidget.auto') : t('musicwidget.px', { n: Math.round(v) })),
    });
    return [
      card(toggleRow('musicwidget.enabled', t('musicwidget.enable.title'), t('musicwidget.enable.desc'))),
      sectionTitle(t('sec.musicwidget.style')),
      wrap,
      sectionTitle(t('sec.musicwidget.glass')),
      card(
        selectRow({
          path: 'musicwidget.material', title: t('musicwidget.material.title'), desc: t('musicwidget.material.desc'), when: on,
          options: () => ['glass', 'clear', 'dark', 'accent'].map((v) => ({ value: v, label: t('musicwidget.material.' + v) })),
        }),
        toggleRow('musicwidget.ambient', t('musicwidget.ambient.title'), t('musicwidget.ambient.desc'), { when: on }),
        sliderRow({
          path: 'musicwidget.opacity', title: t('musicwidget.opacity.title'), desc: t('musicwidget.opacity.desc'), min: 0.3, max: 1, step: 0.05,
          format: (v) => Math.round(v * 100) + '%',
        })),
      sectionTitle(t('sec.musicwidget.place')),
      card(
        selectRow({
          path: 'musicwidget.layer', title: t('musicwidget.layer.title'), desc: t('musicwidget.layer.desc'), when: on,
          options: () => ['desktop', 'top', 'always', 'lockscreen'].map((v) => ({ value: v, label: t('musicwidget.layer.' + v) })),
        }),
        toggleRow('musicwidget.hide_idle', t('musicwidget.hide_idle.title'), t('musicwidget.hide_idle.desc'), { when: on }),
        size('musicwidget.width', 'musicwidget.width', 180, 700),
        size('musicwidget.height', 'musicwidget.height', 56, 600),
        row(t('musicwidget.reset.title'), t('musicwidget.reset.desc'),
          button(t('musicwidget.reset'), () => send({ t: 'action', name: 'musicwidget_reset' }), { small: true }), { when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    for (const code of ['en', 'vi']) {
      setLang(code);
      go('musicwidget');
      const cards = [...document.querySelectorAll('.mw-cards .style-card')];
      expect(cards.length === 5, `${code}: a card for every style`);
      expect(cards.filter((b) => b.getAttribute('aria-pressed') === 'true').length === 1, `${code}: exactly one style is chosen`);
      for (const id of ['card', 'cover', 'bar', 'vinyl', 'minimal']) expect(t('musicwidget.style.' + id) !== 'musicwidget.style.' + id, `${code}: ${id} has a name`);
    }
    setLang('en');
    go('musicwidget');
    sent.length = 0;
    document.querySelectorAll('.mw-cards .style-card')[3].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'musicwidget.style' && m.value === 'vinyl'), 'a style is chosen: ' + JSON.stringify(sent));
    const first = document.querySelector('.switch');
    const was = S.musicwidget.enabled;
    sent.length = 0;
    first.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'musicwidget.enabled' && m.value === !was), 'the switch sends its setting: ' + JSON.stringify(sent));
    first.click();
  },
});
