// Present: a tab of the notch that shows a display, an area, a window or an app to an audience on another display or
// in a window of its own, with a zoom that follows the pointer, a spotlight, a ring around the pointer, a held
// picture, a black or white screen, and keys for all of it. The presenting itself runs in the app (features/present in
// Rust).
(function () {
  // The keys, in the order of the page: the one that starts first, then the ones that work while presenting.
  const ACTIONS = ['start', 'zoom_in', 'zoom_out', 'zoom_reset', 'auto', 'spotlight', 'ring', 'freeze', 'black', 'white'];
  const on = () => S.present.enabled;

  function accessRow() {
    const status = h('span', { class: 'row-desc' });
    const ask = button(t('present.access.ask'), () => send({ t: 'action', name: 'present_screen_access' }), { small: true });
    reg(() => {
      status.textContent = S.present.screen_access ? t('present.access.ok') : t('present.access.no');
      // The button's own style decides its display, so `hidden` alone would not hide it.
      ask.style.display = S.present.screen_access ? 'none' : '';
    });
    return h('div', { class: 'row' },
      h('div', { class: 'row-text' }, h('div', { class: 'row-title' }, t('present.access.title')), status),
      h('div', { class: 'row-control' }, ask));
  }

  (window.__plugins = window.__plugins || []).push({
    id: 'present',
    icon: 'rectangle.on.rectangle',
    en: {
      'nav.present': 'Present', 'page.present': 'Present',
      'page.present.sub': 'Show a display, an area, a window or an app to an audience on another display, with a zoom that follows your pointer.',
      'present.enable.title': 'Show the Present tab', 'present.enable.desc': 'Choose on the tab what to present and where, and start: the audience sees it full screen on the other display, a projector or a TV, or in a window of its own that a video call can share, while the rest of your screen stays yours. The notch and every other window of Figghost are never in the picture; a notification that appears on a display that is shown is, so turn on Do Not Disturb before a talk.',
      'present.access.title': 'Screen Recording', 'present.access.ok': 'Allowed', 'present.access.no': 'Not allowed yet: the picture cannot be shown without it.', 'present.access.ask': 'Allow',
      'sec.present.what': 'What and where',
      'present.source.title': 'What the tab offers first', 'present.source.desc': 'A display shows all of it; an area is dragged out on the frozen screen; a window is shown on its own even while other windows cover it; an app shows all its windows on its display and nothing else.',
      'present.source.display': 'A display', 'present.source.area': 'An area', 'present.source.window': 'A window', 'present.source.app': 'An app',
      'present.fps.title': 'Frames a second', 'present.fps.desc': '60 for video and smooth scrolling, 30 to spare the battery on slides.',
      'present.cursor.title': 'Show the pointer', 'present.cursor.desc': 'The pointer is in the picture, where it is on your screen.',
      'present.border.title': 'Frame what is shown', 'present.border.desc': 'A thin green frame on your screen around the area or display the audience sees. The audience does not see it.',
      'sec.present.zoom': 'Zoom that follows the pointer',
      'present.auto.title': 'Zoom in where you click', 'present.auto.desc': 'A click zooms the audience\'s picture in on it, the view follows the pointer once it nears an edge, and after a quiet moment it goes back to the whole picture, as screen recorders that zoom by themselves do. The zoom keys step in and out around the pointer at any time.',
      'present.zoom.title': 'Zoom of a click', 'present.zoom.desc': 'How large a click makes the picture.',
      'present.zoomout.title': 'Back to the whole picture after', 'present.zoomout.desc': 'Seconds without a click. Typing does not count, so choose longer for live demos.', 'present.zoomout.never': 'Never by itself', 'present.seconds': '{n} seconds',
      'sec.present.pointer': 'Pointer',
      'present.ring.title': 'Ring the pointer from the start', 'present.ring.desc': 'A yellow ring that the audience can follow, as a laser pointer is. It can also be turned on and off while presenting.',
      'present.clicks.title': 'Show clicks', 'present.clicks.desc': 'A ripple where the mouse is clicked.',
      'present.spot.title': 'Size of the spotlight', 'present.spot.desc': 'The spotlight darkens all but a circle around the pointer. It is turned on while presenting, from the tab or with its key.',
      'present.spot.small': 'Small', 'present.spot.medium': 'Medium', 'present.spot.large': 'Large',
      'sec.present.notch': 'On the notch',
      'present.closed.title': 'Show the time in the closed notch', 'present.closed.desc': 'While presenting, how long it has run sits beside the cutout, green while the audience sees the picture live and orange while it is held or covered. A click on it opens the tab.',
      'sec.present.keys': 'Keys',
      'present.keys.desc': 'The key that starts works in any app while the feature is on. The others work only while a presentation runs, so they take no key from your apps the rest of the time. The letters follow the slide shows where they can: B and W for a black and a white screen as in PowerPoint and Keynote, Z for zoom as in ZoomIt and the macOS zoom. Press Delete while recording to turn one off.',
      'present.key.start': 'Start or stop', 'present.key.start.desc': 'Starts with what the tab chose, or stops.',
      'present.key.zoom_in': 'Zoom in', 'present.key.zoom_in.desc': 'One step closer, around the pointer: 1.5, 2, 2.5, 3, 4, 6 and 8 times.',
      'present.key.zoom_out': 'Zoom out', 'present.key.zoom_out.desc': 'One step back.',
      'present.key.zoom_reset': 'Whole picture', 'present.key.zoom_reset.desc': 'Back to all of it at once.',
      'present.key.auto': 'Zoom in where you click, on or off',
      'present.key.spotlight': 'Spotlight', 'present.key.spotlight.desc': 'Darkens all but a circle around the pointer.',
      'present.key.ring': 'Pointer ring',
      'present.key.freeze': 'Hold the picture', 'present.key.freeze.desc': 'The audience keeps seeing the last picture while you look something up; press again to go live.',
      'present.key.black': 'Black screen', 'present.key.black.desc': 'As B in a slide show; press again to come back.',
      'present.key.white': 'White screen', 'present.key.white.desc': 'As W in a slide show.',
      'target.present:start': 'the key that starts presenting', 'target.present:zoom_in': 'the zoom in key', 'target.present:zoom_out': 'the zoom out key',
      'target.present:zoom_reset': 'the whole picture key', 'target.present:auto': 'the automatic zoom key', 'target.present:spotlight': 'the spotlight key',
      'target.present:ring': 'the pointer ring key', 'target.present:freeze': 'the hold key', 'target.present:black': 'the black screen key', 'target.present:white': 'the white screen key',
    },
    vi: {
      'nav.present': 'Trình chiếu', 'page.present': 'Trình chiếu',
      'page.present.sub': 'Chiếu một màn hình, một vùng, một cửa sổ hay một ứng dụng cho khán giả lên màn hình khác, có phóng to bám theo con trỏ.',
      'present.enable.title': 'Hiện tab Trình chiếu', 'present.enable.desc': 'Chọn trong tab chiếu gì và chiếu lên đâu rồi bấm bắt đầu: khán giả thấy nội dung đó toàn màn hình trên màn hình kia, máy chiếu hay TV, hoặc trong một cửa sổ riêng để chia sẻ qua cuộc gọi video, còn phần còn lại của màn hình vẫn là của bạn. Notch và mọi cửa sổ khác của Figghost không bao giờ lọt vào hình; còn thông báo hiện trên màn hình đang chiếu thì có, nên hãy bật Không làm phiền trước buổi thuyết trình.',
      'present.access.title': 'Ghi màn hình', 'present.access.ok': 'Đã cho phép', 'present.access.no': 'Chưa cho phép: không có quyền này thì không hiện được hình.', 'present.access.ask': 'Cho phép',
      'sec.present.what': 'Chiếu gì và chiếu lên đâu',
      'present.source.title': 'Lựa chọn đầu tiên trong tab', 'present.source.desc': 'Màn hình thì chiếu toàn bộ; vùng thì kéo chọn trên màn hình đóng băng; cửa sổ thì chiếu riêng cửa sổ đó kể cả khi bị che; ứng dụng thì chiếu mọi cửa sổ của nó trên màn hình ấy và không gì khác.',
      'present.source.display': 'Một màn hình', 'present.source.area': 'Một vùng', 'present.source.window': 'Một cửa sổ', 'present.source.app': 'Một ứng dụng',
      'present.fps.title': 'Số khung hình mỗi giây', 'present.fps.desc': '60 cho video và cuộn mượt, 30 để tiết kiệm pin khi chiếu slide.',
      'present.cursor.title': 'Hiện con trỏ', 'present.cursor.desc': 'Con trỏ có trong hình, đúng chỗ nó đang ở trên màn hình của bạn.',
      'present.border.title': 'Viền quanh phần đang chiếu', 'present.border.desc': 'Một viền xanh mảnh trên màn hình của bạn bao quanh vùng hay màn hình khán giả đang thấy. Khán giả không thấy viền này.',
      'sec.present.zoom': 'Phóng to bám theo con trỏ',
      'present.auto.title': 'Phóng to chỗ vừa bấm chuột', 'present.auto.desc': 'Mỗi cú bấm phóng to hình của khán giả vào đúng chỗ đó, khung nhìn đi theo con trỏ khi nó gần tới mép, và sau một lúc yên thì trở về toàn cảnh, giống các phần mềm quay màn hình tự phóng to. Phím phóng to, thu nhỏ dùng được bất cứ lúc nào, quanh vị trí con trỏ.',
      'present.zoom.title': 'Mức phóng khi bấm', 'present.zoom.desc': 'Một cú bấm làm hình to lên bao nhiêu lần.',
      'present.zoomout.title': 'Trở về toàn cảnh sau', 'present.zoomout.desc': 'Số giây không bấm chuột. Gõ phím không được tính, nên khi demo trực tiếp hãy chọn lâu hơn.', 'present.zoomout.never': 'Không tự trở về', 'present.seconds': '{n} giây',
      'sec.present.pointer': 'Con trỏ',
      'present.ring.title': 'Khoanh vòng con trỏ ngay từ đầu', 'present.ring.desc': 'Một vòng vàng để khán giả dễ theo dõi, như bút chỉ laser. Trong lúc chiếu vẫn bật tắt được.',
      'present.clicks.title': 'Hiện cú bấm chuột', 'present.clicks.desc': 'Một gợn sóng tại chỗ vừa bấm chuột.',
      'present.spot.title': 'Cỡ đèn chiếu', 'present.spot.desc': 'Đèn chiếu làm tối mọi thứ trừ một vòng tròn quanh con trỏ. Bật trong lúc chiếu, từ tab hoặc bằng phím của nó.',
      'present.spot.small': 'Nhỏ', 'present.spot.medium': 'Vừa', 'present.spot.large': 'Lớn',
      'sec.present.notch': 'Trên notch',
      'present.closed.title': 'Hiện thời gian trong notch đóng', 'present.closed.desc': 'Trong lúc chiếu, thời gian đã chiếu nằm cạnh phần khuyết, màu xanh khi khán giả đang thấy hình trực tiếp và màu cam khi đang đứng hình hoặc bị che. Bấm vào đó là mở tab.',
      'sec.present.keys': 'Phím tắt',
      'present.keys.desc': 'Phím bắt đầu dùng được trong mọi ứng dụng khi tính năng đang bật. Các phím còn lại chỉ có tác dụng trong lúc đang chiếu, nên lúc khác không chiếm phím nào của ứng dụng của bạn. Chữ cái theo thói quen của phần mềm trình chiếu khi có thể: B và W cho màn hình đen và trắng như PowerPoint và Keynote, Z để phóng to như ZoomIt và tính năng phóng to của macOS. Nhấn Delete khi đang ghi phím để tắt phím đó.',
      'present.key.start': 'Bắt đầu hoặc dừng', 'present.key.start.desc': 'Bắt đầu theo lựa chọn trong tab, hoặc dừng.',
      'present.key.zoom_in': 'Phóng to', 'present.key.zoom_in.desc': 'Gần thêm một bậc quanh con trỏ: 1,5, 2, 2,5, 3, 4, 6 và 8 lần.',
      'present.key.zoom_out': 'Thu nhỏ', 'present.key.zoom_out.desc': 'Lùi lại một bậc.',
      'present.key.zoom_reset': 'Toàn cảnh', 'present.key.zoom_reset.desc': 'Trở về toàn bộ hình ngay lập tức.',
      'present.key.auto': 'Bật tắt phóng to chỗ vừa bấm',
      'present.key.spotlight': 'Đèn chiếu', 'present.key.spotlight.desc': 'Làm tối mọi thứ trừ vòng tròn quanh con trỏ.',
      'present.key.ring': 'Vòng con trỏ',
      'present.key.freeze': 'Đứng hình', 'present.key.freeze.desc': 'Khán giả vẫn thấy hình cuối cùng trong lúc bạn tra cứu gì đó; bấm lần nữa để chiếu trực tiếp lại.',
      'present.key.black': 'Màn hình đen', 'present.key.black.desc': 'Như phím B khi trình chiếu slide; bấm lần nữa để quay lại.',
      'present.key.white': 'Màn hình trắng', 'present.key.white.desc': 'Như phím W khi trình chiếu slide.',
      'target.present:start': 'phím bắt đầu trình chiếu', 'target.present:zoom_in': 'phím phóng to', 'target.present:zoom_out': 'phím thu nhỏ',
      'target.present:zoom_reset': 'phím toàn cảnh', 'target.present:auto': 'phím tự phóng to', 'target.present:spotlight': 'phím đèn chiếu',
      'target.present:ring': 'phím vòng con trỏ', 'target.present:freeze': 'phím đứng hình', 'target.present:black': 'phím màn hình đen', 'target.present:white': 'phím màn hình trắng',
    },
    build() {
      const when = { when: on };
      const pick = (path, key, options) => selectRow({ path: 'present.' + path, title: t('present.' + key + '.title'), desc: t('present.' + key + '.desc'), options, when: on });
      const sources = () => ['display', 'area', 'window', 'app'].map((v) => ({ value: v, label: t('present.source.' + v) }));
      const fps = () => [30, 60].map((n) => ({ value: n, label: String(n) }));
      const zooms = () => [1.5, 2, 2.5, 3, 4].map((n) => ({ value: n, label: String(n).replace('.', lang === 'vi' ? ',' : '.') + 'x' }));
      const outs = () => [0, 2, 3, 5, 8].map((n) => ({ value: n, label: n ? t('present.seconds', { n }) : t('present.zoomout.never') }));
      const spots = () => [[12, 'small'], [18, 'medium'], [26, 'large']].map(([n, k]) => ({ value: n, label: t('present.spot.' + k) }));
      const key = (action) => {
        const desc = I18N.en['present.key.' + action + '.desc'] ? t('present.key.' + action + '.desc') : null;
        return row(t('present.key.' + action), desc, hotkeyButton('present:' + action, { allowClear: true, label: t('present.key.' + action) }), when);
      };
      return [
        card(toggleRow('present.enabled', t('present.enable.title'), t('present.enable.desc'))),
        card(accessRow()),
        sectionTitle(t('sec.present.what')),
        card(
          pick('source', 'source', sources),
          pick('fps', 'fps', fps),
          toggleRow('present.cursor', t('present.cursor.title'), t('present.cursor.desc'), when),
          toggleRow('present.border', t('present.border.title'), t('present.border.desc'), when)),
        sectionTitle(t('sec.present.zoom')),
        card(
          toggleRow('present.auto_zoom', t('present.auto.title'), t('present.auto.desc'), when),
          pick('zoom', 'zoom', zooms),
          pick('zoom_out', 'zoomout', outs)),
        sectionTitle(t('sec.present.pointer')),
        card(
          toggleRow('present.ring', t('present.ring.title'), t('present.ring.desc'), when),
          toggleRow('present.clicks', t('present.clicks.title'), t('present.clicks.desc'), when),
          pick('spotlight', 'spot', spots)),
        sectionTitle(t('sec.present.notch')),
        card(toggleRow('present.closed_notch', t('present.closed.title'), t('present.closed.desc'), when)),
        sectionTitle(t('sec.present.keys')),
        card(
          h('div', { class: 'row-desc', style: 'padding:12px 14px 0', text: t('present.keys.desc') }),
          ...ACTIONS.map(key)),
      ];
    },
    selftest({ expect, go, sent, S }) {
      go('present');
      const page = document.querySelector('#scroller');
      expect(page.querySelectorAll('.hotkey').length === ACTIONS.length, 'present: a key recorder for each action');
      const switches = [...page.querySelectorAll('.switch')];
      expect(switches.length === 7, 'present: the tab, the pointer, the frame, the zoom, the ring, the clicks and the notch: ' + switches.length);
      sent.length = 0;
      const was = S.present.enabled;
      switches[0].click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'present.enabled' && m.value === !was), 'present: the switch sends its setting: ' + JSON.stringify(sent));
      switches[0].click();
      const picks = [...page.querySelectorAll('select.pick')];
      expect(picks.length === 5, 'present: what first, the frames, the zoom, when it goes back, the spotlight');
      sent.length = 0;
      picks[2].value = '2.5';
      picks[2].dispatchEvent(new Event('change'));
      expect(sent.some((m) => m.path === 'present.zoom' && m.value === 2.5), 'present: the zoom is a number: ' + JSON.stringify(sent));
      picks[0].value = 'area';
      picks[0].dispatchEvent(new Event('change'));
      expect(S.present.source === 'area', 'present: what comes first is a word');
      picks[2].value = '2'; picks[2].dispatchEvent(new Event('change'));
      picks[0].value = 'display'; picks[0].dispatchEvent(new Event('change'));
      expect(page.textContent.includes(t('present.access.ok')), 'present: the page says Screen Recording is allowed');
    },
  });
})();
