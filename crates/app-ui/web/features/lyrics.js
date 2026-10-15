// Lyrics on the media tab: two switches, as cards at the end of the Notch page.
(window.__plugins = window.__plugins || []).push({
  id: 'lyrics',
  en: {
    'lyrics.sec': 'Lyrics',
    'lyrics.show.title': 'Show lyrics',
    'lyrics.show.desc': 'Shows the words of the song on the Media tab, line by line as it plays. Works with Music and Spotify.',
    'lyrics.fetch.title': 'Fetch lyrics online',
    'lyrics.fetch.desc': 'Looks lyrics up on lrclib.net, a free service. To do that it sends the artist, track and album names and the length of the track to lrclib.net, and nothing else. Lyrics that are found are kept on this Mac, so a song is asked about once; a song without lyrics is asked about again after a few days. With this off, only lyrics already kept are shown.',
  },
  vi: {
    'lyrics.sec': 'Lời bài hát',
    'lyrics.show.title': 'Hiện lời bài hát',
    'lyrics.show.desc': 'Hiện lời bài hát trên tab Media, từng dòng theo bài đang phát. Dùng được với Music và Spotify.',
    'lyrics.fetch.title': 'Tải lời bài hát trực tuyến',
    'lyrics.fetch.desc': 'Tra lời bài hát trên lrclib.net, một dịch vụ miễn phí. Để làm việc đó, ứng dụng gửi tên nghệ sĩ, tên bài hát, tên album và độ dài bài hát tới lrclib.net, không gửi gì khác. Lời tìm được sẽ lưu trên máy Mac này nên mỗi bài chỉ hỏi một lần; bài chưa có lời sẽ được hỏi lại sau vài ngày. Khi tắt mục này, chỉ hiện những lời đã lưu sẵn.',
  },
  slots: {
    notch() {
      return [
        sectionTitle(t('lyrics.sec')),
        card(
          toggleRow('lyrics.enabled', t('lyrics.show.title'), t('lyrics.show.desc')),
          toggleRow('lyrics.fetch', t('lyrics.fetch.title'), t('lyrics.fetch.desc'))),
      ];
    },
  },
  selftest({ expect, go, sent, S, $ }) {
    for (const code of ['en', 'vi']) {
      setLang(code);
      go('notch');
      const root = $('#scroller');
      const find = (label) => [...root.querySelectorAll('.switch')].find((b) => b.getAttribute('aria-label') === label);
      const show = find(t('lyrics.show.title'));
      const fetchSwitch = find(t('lyrics.fetch.title'));
      expect(show && fetchSwitch, `${code}: the Notch page has both lyrics switches`);
      if (!show || !fetchSwitch) continue;
      expect(/lrclib\.net/.test(root.textContent), `${code}: the privacy text names lrclib.net`);
      expect(/\b(artist|nghệ sĩ)/i.test(t('lyrics.fetch.desc')) && /(album)/i.test(t('lyrics.fetch.desc')), `${code}: the privacy text lists what is sent`);
      // Changes to one setting are batched for a moment, so the sending is checked once, in English.
      if (code !== 'en') continue;
      for (const [el, path] of [[show, 'lyrics.enabled'], [fetchSwitch, 'lyrics.fetch']]) {
        const before = S.lyrics[path.split('.')[1]];
        sent.length = 0;
        el.click();
        expect(sent.some((m) => m.t === 'set' && m.path === path && m.value === !before), `${code}: ${path} sends its change: ${JSON.stringify(sent)}`);
        el.click();
      }
    }
    setLang('en');
  },
});
