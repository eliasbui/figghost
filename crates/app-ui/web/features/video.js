// Video (beta): a tab of the notch that plays one stream or video file, muted.
(window.__plugins = window.__plugins || []).push({
  id: 'video',
  icon: 'play.rectangle.fill',
  en: {
    'nav.video': 'Video (beta)', 'page.video': 'Video (beta)',
    'page.video.sub': 'Play a live stream or a video in a tab of the notch.',
    'video.enable.title': 'Show the Video tab', 'video.enable.desc': 'Beta. Plays the address below in the open notch, without sound. It plays only while the tab shows, so a closed notch fetches nothing. HLS (.m3u8), MP4 and MOV over https, or a local file, are the kinds macOS plays.',
    'sec.video.source': 'Source',
    'video.url.title': 'Address', 'video.url.desc': 'An https address, or file:// for a video on this Mac. Press Enter to save it.',
    'video.url.ph': 'https://example.com/live/stream.m3u8',
  },
  vi: {
    'nav.video': 'Video (beta)', 'page.video': 'Video (beta)',
    'page.video.sub': 'Phát một luồng trực tiếp hoặc một video trong tab của notch.',
    'video.enable.title': 'Hiện tab Video', 'video.enable.desc': 'Bản beta. Phát địa chỉ bên dưới trong notch đang mở, không có tiếng. Chỉ phát khi tab đang hiện, nên notch đóng thì không tải gì. macOS phát được HLS (.m3u8), MP4 và MOV qua https, hoặc một tệp trên máy.',
    'sec.video.source': 'Nguồn',
    'video.url.title': 'Địa chỉ', 'video.url.desc': 'Địa chỉ https, hoặc file:// cho video trên máy Mac này. Nhấn Enter để lưu.',
    'video.url.ph': 'https://example.com/live/stream.m3u8',
  },
  build() {
    const on = () => S.video.enabled;
    const field = h('input', { type: 'text', class: 'text-field', 'aria-label': t('video.url.title'), placeholder: t('video.url.ph'), maxlength: '2048', spellcheck: 'false', autocomplete: 'off', autocapitalize: 'off' });
    const save = () => change('video.url', field.value.trim());
    field.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.isComposing) save(); });
    field.addEventListener('change', save);
    // A field that is being typed in is not overwritten by the state that comes every two seconds.
    reg(() => { if (document.activeElement !== field) field.value = S.video.url; });
    return [
      card(toggleRow('video.enabled', t('video.enable.title'), t('video.enable.desc'))),
      sectionTitle(t('sec.video.source')),
      card(row(t('video.url.title'), t('video.url.desc'), field, { block: true, when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('video');
    const page = document.querySelector('#scroller');
    const switches = [...page.querySelectorAll('.switch')];
    expect(switches.length === 1, 'video: one switch for the tab');
    sent.length = 0;
    const was = S.video.enabled;
    switches[0].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'video.enabled' && m.value === !was), 'video: the switch sends its setting: ' + JSON.stringify(sent));
    switches[0].click();
    const field = page.querySelector('input.text-field');
    expect(field && field.value === S.video.url, 'video: the field shows the address');
    sent.length = 0;
    field.value = ' https://example.com/a.mp4 ';
    field.dispatchEvent(new Event('change'));
    expect(S.video.url === 'https://example.com/a.mp4', 'video: the address is trimmed');
  },
});
