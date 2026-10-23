// Video download: paste a link and yt-dlp, which you installed yourself, saves the video to your Downloads folder. The
// state is under S.download; the progress comes with it every two seconds while the page is open, and the app checks every
// link again.
(window.__plugins = window.__plugins || []).push({
  id: 'download',
  icon: 'arrow.down.circle.fill',
  en: {
    'nav.download': 'Video download', 'page.download': 'Video download',
    'page.download.sub': 'Paste a link to a video and save it to your Mac. The work is done by yt-dlp, which you install yourself.',
    'download.enable.title': 'Download videos', 'download.enable.desc': 'Adds a Download video entry to the menu bar panel that takes the link you copied, and shows the progress on the closed notch. Figghost does not include yt-dlp; it uses your copy. Only save videos you have the right to keep.',
    'sec.download.tools': 'Tools',
    'download.ytdlp.title': 'yt-dlp', 'download.ytdlp.found': 'Found, version {version}', 'download.ytdlp.found.plain': 'Found',
    'download.ytdlp.missing': 'Not installed. In Terminal, run: brew install yt-dlp',
    'download.ffmpeg.title': 'ffmpeg', 'download.ffmpeg.found': 'Found. It joins video and sound, so above 1080p (VP9, AV1) and other formats work, and music only is saved as m4a.',
    'download.ffmpeg.missing': 'Not installed. Figghost joins H.264 video and AAC sound itself, so YouTube works up to 1080p. ffmpeg is needed only above 1080p (VP9, AV1), for sites with formats Figghost cannot join, and to turn music into m4a. To get it, run: brew install ffmpeg',
    'sec.download.get': 'Download',
    'download.link.label': 'Link to the video', 'download.link.ph': 'https://...', 'download.go': 'Download',
    'download.quality.title': 'Quality', 'download.quality.desc': 'The best the site has, or no taller than the height you choose. Music only saves the sound.',
    'download.quality.best': 'Best', 'download.quality.1080': 'Up to 1080p', 'download.quality.720': 'Up to 720p', 'download.quality.audio': 'Music only',
    'download.playlist.title': 'Whole playlist', 'download.playlist.desc': 'A link to a playlist saves all of it. Off saves only the video the link points at.',
    'download.folder.title': 'Save to', 'download.folder.choose': 'Choose...', 'download.folder.reset': 'Use Downloads',
    'sec.download.list': 'Downloads',
    'download.list.empty': 'Nothing downloaded yet. Paste a link above.',
    'download.clear': 'Clear finished',
    'download.cancel': 'Cancel', 'download.reveal': 'Show in Finder',
    'download.st.queued': 'Waiting for a free place', 'download.st.running': 'Starting...', 'download.st.joining': 'Joining', 'download.st.done': 'Saved', 'download.st.cancelled': 'Cancelled',
    'download.st.failed': 'Failed',
    'download.err.ffmpeg': 'This video needs ffmpeg: Figghost cannot join its formats itself. In Terminal, run: brew install ffmpeg',
    'download.speed': '{speed}/s', 'download.eta': '{time} left',
    'fault.download_off': 'Turn on Download videos first.',
    'fault.download_missing': 'yt-dlp is not installed. In Terminal, run: brew install yt-dlp',
    'fault.download_bad_url': 'That is not a link that starts with http:// or https://.',
  },
  vi: {
    'nav.download': 'Tải video', 'page.download': 'Tải video',
    'page.download.sub': 'Dán link video để lưu về máy Mac. Việc tải do yt-dlp làm, công cụ bạn tự cài.',
    'download.enable.title': 'Tải video', 'download.enable.desc': 'Thêm mục Tải video vào bảng ở thanh menu, lấy link bạn vừa sao chép, và hiện tiến độ trên notch đóng. Figghost không kèm yt-dlp mà dùng bản bạn đã cài. Chỉ lưu những video bạn có quyền lưu giữ.',
    'sec.download.tools': 'Công cụ',
    'download.ytdlp.title': 'yt-dlp', 'download.ytdlp.found': 'Đã có, phiên bản {version}', 'download.ytdlp.found.plain': 'Đã có',
    'download.ytdlp.missing': 'Chưa cài. Trong Terminal, chạy: brew install yt-dlp',
    'download.ffmpeg.title': 'ffmpeg', 'download.ffmpeg.found': 'Đã có. ffmpeg ghép hình và tiếng nên tải được trên 1080p (VP9, AV1) và các định dạng khác, và chỉ lấy nhạc thì lưu thành m4a.',
    'download.ffmpeg.missing': 'Chưa cài. Figghost tự ghép hình H.264 và tiếng AAC nên YouTube tải được tối đa 1080p. Chỉ cần ffmpeg khi muốn trên 1080p (VP9, AV1), với trang có định dạng Figghost không tự ghép được, và để chuyển nhạc sang m4a. Muốn cài, hãy chạy: brew install ffmpeg',
    'sec.download.get': 'Tải về',
    'download.link.label': 'Link video', 'download.link.ph': 'https://...', 'download.go': 'Tải',
    'download.quality.title': 'Chất lượng', 'download.quality.desc': 'Tốt nhất mà trang có, hoặc không cao hơn chiều cao bạn chọn. Chỉ lấy nhạc thì chỉ lưu phần tiếng.',
    'download.quality.best': 'Tốt nhất', 'download.quality.1080': 'Đến 1080p', 'download.quality.720': 'Đến 720p', 'download.quality.audio': 'Chỉ lấy nhạc',
    'download.playlist.title': 'Cả danh sách phát', 'download.playlist.desc': 'Link tới một danh sách phát thì lưu toàn bộ. Tắt thì chỉ lưu video mà link trỏ tới.',
    'download.folder.title': 'Lưu vào', 'download.folder.choose': 'Chọn...', 'download.folder.reset': 'Dùng Downloads',
    'sec.download.list': 'Các lượt tải',
    'download.list.empty': 'Chưa tải gì. Hãy dán link ở trên.',
    'download.clear': 'Xóa các lượt đã xong',
    'download.cancel': 'Hủy', 'download.reveal': 'Mở trong Finder',
    'download.st.queued': 'Đang chờ đến lượt', 'download.st.running': 'Đang bắt đầu...', 'download.st.joining': 'Đang ghép', 'download.st.done': 'Đã lưu', 'download.st.cancelled': 'Đã hủy',
    'download.st.failed': 'Thất bại',
    'download.err.ffmpeg': 'Video này cần ffmpeg: Figghost không tự ghép được định dạng của nó. Trong Terminal, chạy: brew install ffmpeg',
    'download.speed': '{speed}/giây', 'download.eta': 'còn {time}',
    'fault.download_off': 'Hãy bật Tải video trước.',
    'fault.download_missing': 'Chưa cài yt-dlp. Trong Terminal, chạy: brew install yt-dlp',
    'fault.download_bad_url': 'Đó không phải link bắt đầu bằng http:// hoặc https://.',
  },
  build() {
    const dl = () => S.download;
    const on = () => dl().enabled;
    const ready = () => on() && !!dl().ytdlp;
    const act = (name) => send({ t: 'action', name });
    const bytes = (n) => {
      const units = ['B', 'KB', 'MB', 'GB'];
      const i = Math.min(units.length - 1, Math.floor(Math.log(Math.max(n, 1)) / Math.log(1024)));
      return (n / 1024 ** i).toFixed(i > 1 ? 1 : 0) + ' ' + units[i];
    };
    const clock = (s) => Math.floor(s / 60) + ':' + String(s % 60).padStart(2, '0');
    const hostOf = (url) => url.replace(/^[a-z]+:\/\//i, '').split(/[/?#]/)[0];

    // Tools ---------------------------------------------------------
    const ytdlp = h('span', { class: 'download-state' });
    const ffmpeg = h('span', { class: 'download-state' });
    const ytdlpRow = row(t('download.ytdlp.title'), ' ', ytdlp);
    const ffmpegRow = row(t('download.ffmpeg.title'), ' ', ffmpeg);
    reg(() => {
      const d = dl();
      ytdlpRow.querySelector('.row-desc').textContent = d.ytdlp ? (d.ytdlp_version ? t('download.ytdlp.found', { version: d.ytdlp_version }) : t('download.ytdlp.found.plain')) : t('download.ytdlp.missing');
      ytdlp.textContent = d.ytdlp ? d.ytdlp : '';
      ytdlpRow.dataset.state = d.ytdlp ? 'ready' : 'missing';
      ffmpegRow.querySelector('.row-desc').textContent = d.ffmpeg ? t('download.ffmpeg.found') : t('download.ffmpeg.missing');
      ffmpeg.textContent = d.ffmpeg ? d.ffmpeg : '';
      ffmpegRow.dataset.state = d.ffmpeg ? 'ready' : 'missing';
    });

    // The link ------------------------------------------------------
    const field = h('input', { type: 'text', class: 'text-field', 'aria-label': t('download.link.label'), placeholder: t('download.link.ph'), maxlength: '2048', spellcheck: 'false', autocomplete: 'off', autocapitalize: 'off' });
    const go = button(t('download.go'), () => submit(), { primary: true, small: true, icon: 'arrow.down.circle.fill' });
    const submit = () => {
      const link = field.value.trim();
      if (!/^https?:\/\/[^\s\u0000-\u001f]+$/i.test(link)) { toast(t('fault.download_bad_url')); return; }
      act('download_add:' + link);
      field.value = '';
    };
    field.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.isComposing) submit(); });
    reg(() => { go.disabled = !ready(); field.disabled = !ready(); });
    const linkRow = h('div', { class: 'row block' }, h('div', { class: 'download-link' }, field, go));

    const qualities = () => ['best', '1080', '720', 'audio'].map((q) => ({ value: q, label: t('download.quality.' + q) }));

    // The folder ----------------------------------------------------
    const where = h('span', { class: 'download-path' });
    const reset = button(t('download.folder.reset'), () => change('download.folder', ''), { small: true });
    const choose = button(t('download.folder.choose'), () => act('download_pick'), { small: true });
    reg(() => { where.textContent = dl().folder_path; reset.hidden = !dl().folder; });
    // The panel that chooses a folder is the app's, so the choice comes back through the state and is stored from here.
    let seenPick = dl().picked.seq;
    reg(() => {
      const p = dl().picked;
      if (p.seq === seenPick) return;
      seenPick = p.seq;
      if (p.path) change('download.folder', p.path);
    });
    const folderRow = row(t('download.folder.title'), null, h('div', { class: 'download-folder' }, where, reset, choose), { when: on });
    folderRow.querySelector('.row-text').after(h('div', { class: 'download-gap' }));

    // The list ------------------------------------------------------
    const box = h('div', { class: 'download-list' });
    const empty = h('div', { class: 'empty', role: 'status', text: t('download.list.empty') });
    const clear = button(t('download.clear'), () => act('download_clear'), { small: true });
    const rows = new Map();
    function jobRow(j) {
      const name = h('span', { class: 'download-name' });
      const note = h('span', { class: 'download-note' });
      const fill = h('span');
      const bar = h('span', { class: 'download-bar', role: 'progressbar', 'aria-valuemin': '0', 'aria-valuemax': '100' }, fill);
      const stop = button(t('download.cancel'), () => act('download_cancel:' + j.id), { small: true, icon: 'xmark' });
      const show = button(t('download.reveal'), () => act('download_reveal:' + j.id), { small: true, icon: 'folder' });
      const el = h('div', { class: 'download-item' }, h('span', { class: 'download-text' }, name, note, bar), h('span', { class: 'download-tools' }, stop, show));
      const update = (j) => {
        el.dataset.status = j.status;
        name.textContent = j.title || hostOf(j.url);
        el.title = j.url;
        const pct = j.percent == null ? 0 : j.percent;
        bar.hidden = j.status !== 'running';
        fill.style.width = pct + '%';
        bar.setAttribute('aria-valuenow', String(Math.round(pct)));
        let text = t('download.st.' + j.status);
        if (j.status === 'running' && j.percent != null) {
          text = [Math.round(pct) + '%', j.speed ? t('download.speed', { speed: bytes(j.speed) }) : '', j.eta != null ? t('download.eta', { time: clock(j.eta) }) : ''].filter(Boolean).join(' / ');
        } else if (j.status === 'failed') {
          text = j.needs_ffmpeg ? t('download.err.ffmpeg') : t('download.st.failed') + ': ' + j.error;
        }
        note.textContent = text;
        stop.hidden = !(j.status === 'queued' || j.status === 'running');
        show.hidden = j.status !== 'done';
      };
      return { el, update };
    }
    let order = '';
    reg(() => {
      const jobs = dl().jobs;
      const ids = jobs.map((j) => j.id).join(',') + lang;
      if (ids !== order) {
        order = ids;
        for (const [id, r] of [...rows]) if (!jobs.some((j) => j.id === id)) rows.delete(id);
        for (const j of jobs) if (!rows.has(j.id)) rows.set(j.id, jobRow(j));
        box.replaceChildren(...(jobs.length ? [...jobs].reverse().map((j) => rows.get(j.id).el) : [empty]));
      }
      for (const j of jobs) rows.get(j.id).update(j);
      clear.disabled = !jobs.some((j) => j.status === 'done' || j.status === 'failed' || j.status === 'cancelled');
    });

    return [
      card(toggleRow('download.enabled', t('download.enable.title'), t('download.enable.desc'))),
      sectionTitle(t('sec.download.tools')),
      card(ytdlpRow, ffmpegRow),
      sectionTitle(t('sec.download.get')),
      card(linkRow,
        selectRow({ path: 'download.quality', title: t('download.quality.title'), desc: t('download.quality.desc'), options: qualities, when: on }),
        toggleRow('download.playlist', t('download.playlist.title'), t('download.playlist.desc'), { when: on }),
        folderRow),
      sectionTitle(t('sec.download.list')),
      card(box, h('div', { class: 'download-toolbar' }, clear)),
    ];
  },
  selftest({ expect, go, sent, S }) {
    S.download.enabled = true;
    S.download.ytdlp = '/opt/homebrew/bin/yt-dlp';
    S.download.ytdlp_version = '2026.08.19';
    S.download.ffmpeg = '';
    go('download');
    const page = document.querySelector('#scroller');
    const btn = (label) => [...page.querySelectorAll('.btn')].find((b) => b.textContent === label);
    expect(page.querySelectorAll('.switch').length === 2, 'download: one switch for the feature and one for playlists');
    expect(page.textContent.includes(t('download.ytdlp.found', { version: '2026.08.19' })), 'download: the version of yt-dlp is shown');
    expect(page.textContent.includes('brew install ffmpeg'), 'download: a missing ffmpeg says what it costs and how to install it');
    sent.length = 0;
    page.querySelector('.switch').click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'download.enabled' && m.value === false), 'download: the switch sends its setting: ' + JSON.stringify(sent));
    page.querySelector('.switch').click();

    // A link is sent as an action once it is one, and refused here first when it is not.
    const field = page.querySelector('input.text-field');
    sent.length = 0;
    field.value = 'not a link';
    btn(t('download.go')).click();
    expect(!sent.length, 'download: text that is not a link is not sent: ' + JSON.stringify(sent));
    field.value = ' https://example.com/watch?v=abc ';
    btn(t('download.go')).click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'download_add:https://example.com/watch?v=abc'), 'download: the link is sent: ' + JSON.stringify(sent));
    expect(field.value === '', 'download: the field is emptied once the link is sent');
    field.value = 'https://example.com/enter';
    sent.length = 0;
    field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    expect(sent.some((m) => m.name === 'download_add:https://example.com/enter'), 'download: Enter sends the link: ' + JSON.stringify(sent));

    // Quality.
    const pick = page.querySelector('select.pick');
    expect([...pick.options].map((o) => o.value).join() === 'best,1080,720,audio', 'download: four qualities to choose from');
    pick.value = '720';
    pick.dispatchEvent(new Event('change'));
    expect(S.download.quality === '720', 'download: the quality is chosen');
    pick.value = 'best';
    pick.dispatchEvent(new Event('change'));

    // The list: progress, joining, a finished file, a failure and one that needs ffmpeg.
    const items = [...page.querySelectorAll('.download-item')];
    expect(items.length === S.download.jobs.length, 'download: a row for each download: ' + items.length);
    const running = items.find((i) => i.dataset.status === 'running');
    expect(running && running.textContent.includes('42%'), 'download: the progress of a running download is in its row');
    expect(running.querySelector('.download-bar').getAttribute('aria-valuenow') === '42', 'download: the bar has the percentage');
    sent.length = 0;
    [...running.querySelectorAll('.btn')].find((b) => !b.hidden).click();
    expect(sent.some((m) => m.name === 'download_cancel:' + S.download.jobs.find((j) => j.status === 'running').id), 'download: a running download can be cancelled: ' + JSON.stringify(sent));
    const joining = items.find((i) => i.dataset.status === 'joining');
    expect(joining && joining.textContent.includes(t('download.st.joining')), 'download: a download that joins its parts says so');
    expect(joining && [...joining.querySelectorAll('.btn')].every((b) => b.hidden), 'download: a download that joins its parts offers no button');
    const done = items.find((i) => i.dataset.status === 'done');
    sent.length = 0;
    [...done.querySelectorAll('.btn')].find((b) => !b.hidden).click();
    expect(sent.some((m) => m.name.startsWith('download_reveal:')), 'download: a finished one opens in the Finder: ' + JSON.stringify(sent));
    expect(done.querySelector('.btn:not([hidden])').textContent === t('download.reveal'), 'download: a finished one offers no cancel');
    const failed = items.find((i) => i.dataset.status === 'failed' && !i.textContent.includes('brew install ffmpeg'));
    expect(failed && failed.textContent.includes(S.download.jobs.find((j) => j.status === 'failed').error), 'download: the error of a failed one is shown');
    expect(items.some((i) => i.textContent.includes('brew install ffmpeg')), 'download: a download that needs ffmpeg explains it');
    sent.length = 0;
    btn(t('download.clear')).click();
    expect(sent.some((m) => m.name === 'download_clear'), 'download: finished downloads can be cleared');

    // The progress moves in place: the row is the same element, so a click is not lost to a rebuild.
    S.download.jobs.find((j) => j.status === 'running').percent = 77; runSyncs();
    expect(items.includes(running) && running.textContent.includes('77%') && document.contains(running), 'download: the progress is updated in the same row');

    // The folder: choosing opens the app's panel, and what it chose is stored from here.
    sent.length = 0;
    btn(t('download.folder.choose')).click();
    expect(sent.some((m) => m.name === 'download_pick'), 'download: the folder button asks the app to open its panel: ' + JSON.stringify(sent));
    S.download.picked = { seq: S.download.picked.seq + 1, path: '/Users/me/Movies' }; runSyncs();
    expect(sent.some((m) => m.t === 'set' && m.path === 'download.folder' && m.value === '/Users/me/Movies'), 'download: the chosen folder is stored: ' + JSON.stringify(sent));
    S.download.folder = '/Users/me/Movies'; runSyncs();
    btn(t('download.folder.reset')).click();
    expect(S.download.folder === '', 'download: Use Downloads clears the folder');

    // Without yt-dlp nothing can be started and the page says how to get it.
    const was = S.download.ytdlp;
    S.download.ytdlp = ''; runSyncs();
    expect(page.textContent.includes('brew install yt-dlp'), 'download: a missing yt-dlp says how to install it');
    expect(btn(t('download.go')).disabled && field.disabled, 'download: no link can be sent without yt-dlp');
    S.download.ytdlp = was; runSyncs();
    S.download.enabled = false; runSyncs();
    expect(btn(t('download.go')).disabled, 'download: no link can be sent while the feature is off');
    S.download.enabled = true; runSyncs();
  },
});
