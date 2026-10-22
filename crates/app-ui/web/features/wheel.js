// The drag wheel: a page of its own. While files are dragged, tapping a key opens a wheel of slots around the
// pointer; dropping the files on a slot runs it. There are three wheels (convert formats, tools, actions). Each
// has a switch, the key that opens it and the choice of the slots it shows.
(function () {
  const WHEELS = ['formats', 'tools', 'actions'];
  const KEYS = [['shift', '⇧ Shift'], ['command', '⌘ Command'], ['control', '⌃ Control'], ['option', '⌥ Option']];
  const GROUPS = ['image', 'video', 'audio', 'pdf', 'document', 'any'];
  // Every slot of every wheel as group:id, in the order of catalog::all_ids; a unit test keeps the two in step.
  const SLOTS = {
    formats: [
      'image:to_png', 'image:to_jpeg', 'image:to_heic', 'image:to_tiff', 'image:to_avif', 'image:to_gif', 'image:to_bmp', 'image:to_pdf',
      'video:to_mp4', 'video:to_mov', 'video:to_gif', 'video:to_m4a',
      'audio:to_m4a', 'audio:to_wav', 'audio:to_aiff', 'audio:to_flac', 'audio:to_caf', 'audio:to_aac',
      'pdf:to_png', 'pdf:to_jpeg',
      'document:to_docx', 'document:to_doc', 'document:to_rtf', 'document:to_odt', 'document:to_html', 'document:to_txt', 'document:to_pdf',
      'any:zip', 'any:tar', 'any:tgz',
    ],
    tools: [
      'image:compress', 'image:half', 'image:quarter', 'image:strip_metadata', 'image:remove_background', 'image:copy_text', 'image:annotate',
      'video:compress_1080p', 'video:compress_720p', 'video:compress_540p', 'video:strip_metadata',
      'audio:compress',
      'pdf:pdf_merge', 'pdf:pdf_split',
    ],
    actions: ['any:copy_path', 'any:copy', 'any:airdrop', 'any:terminal', 'any:reveal', 'any:float'],
  };
  const split = (token) => token.split(':');
  const uniqueIds = (wheel) => [...new Set(SLOTS[wheel].map((s) => split(s)[1]))];
  // The names of formats and archives are written the same in every language.
  const isFormat = (id) => id.startsWith('to_') || ['zip', 'tar', 'tgz'].includes(id);
  const slotTitle = (id) => (isFormat(id) ? (id.startsWith('to_') ? id.slice(3) : id).toUpperCase() : t('wheel.slot.' + id));

  // Another wheel that is on and uses this key.
  const takenBy = (wheel, key) => WHEELS.find((o) => o !== wheel && S.wheel[o].enabled && S.wheel[o].key === key);

  function enableSwitch(wheel) {
    const label = () => t('wheel.' + wheel + '.title');
    const el = h('button', { class: 'switch', role: 'switch', 'aria-checked': 'false', 'aria-label': label() });
    el.addEventListener('click', () => {
      const on = !S.wheel[wheel].enabled;
      // The app refuses it too; saying so here keeps the switch from flickering on and back.
      if (on && takenBy(wheel, S.wheel[wheel].key)) return toast(t('fault.wheel_key_taken'));
      change('wheel.' + wheel + '.enabled', on);
    });
    reg(() => { el.setAttribute('aria-checked', String(!!S.wheel[wheel].enabled)); el.setAttribute('aria-label', label()); });
    return el;
  }

  // Any other key: the button records the next one pressed, as a shortcut field does, but alone, without a
  // modifier. Escape gives up, since it is the key that ends a drag; a key no shortcut knows says so.
  function otherKey(wheel) {
    const el = h('button', { class: 'btn small wheel-other', type: 'button' });
    let recording = false;
    const custom = () => !KEYS.some(([value]) => value === S.wheel[wheel].key);
    const render = () => {
      el.classList.toggle('on', custom() && !recording);
      el.classList.toggle('recording', recording);
      el.replaceChildren(recording ? t('wheel.key.press') : custom() ? keycapsEl(S.wheel[wheel].key) : t('wheel.key.other'));
      el.setAttribute('aria-label', t('wheel.key.other') + (custom() ? ': ' + S.wheel[wheel].key : ''));
    };
    const stop = () => {
      recording = false;
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('mousedown', onAway, true);
      send({ t: 'recording', on: false });
      render();
    };
    const onAway = (e) => { if (!el.contains(e.target)) stop(); };
    const onKey = (e) => {
      e.preventDefault();
      e.stopPropagation();
      // A modifier alone is one of the four choices beside the button.
      if (MODIFIER_CODES.has(e.code)) return;
      if (e.code === 'Escape') return stop();
      const key = keyName(e.code);
      if (!key) return toast(t('fault.unsupported_key'));
      stop();
      if (key === S.wheel[wheel].key) return;
      if (S.wheel[wheel].enabled && takenBy(wheel, key)) return toast(t('fault.wheel_key_taken'));
      change('wheel.' + wheel + '.key', key);
    };
    el.addEventListener('click', () => {
      if (recording) return stop();
      recording = true;
      // Global shortcuts let go while a key is recorded, so the key reaches the page.
      send({ t: 'recording', on: true });
      window.addEventListener('keydown', onKey, true);
      window.addEventListener('mousedown', onAway, true);
      render();
    });
    reg(render);
    return el;
  }

  function keyChoice(wheel) {
    return h('div', { class: 'wheel-key' }, modifierChoice(wheel), otherKey(wheel));
  }

  function modifierChoice(wheel) {
    return segmented({
      label: t('wheel.key.title'),
      options: KEYS.map(([value, label]) => ({ value, label })),
      value: () => S.wheel[wheel].key,
      onPick: (key) => {
        if (key === S.wheel[wheel].key) return;
        if (S.wheel[wheel].enabled && takenBy(wheel, key)) return toast(t('fault.wheel_key_taken'));
        change('wheel.' + wheel + '.key', key);
      },
    });
  }

  function pick(wheel, id) {
    const el = h('button', { class: 'wheel-pick', type: 'button', 'aria-pressed': 'false' }, h('span', { class: 'wheel-name', text: slotTitle(id) }));
    el.dataset.id = id;
    el.addEventListener('click', () => {
      const hidden = new Set(S.wheel[wheel].hidden);
      if (hidden.has(id)) hidden.delete(id); else hidden.add(id);
      change('wheel.' + wheel + '.hidden', uniqueIds(wheel).filter((i) => hidden.has(i)));
    });
    reg(() => {
      el.setAttribute('aria-pressed', String(!S.wheel[wheel].hidden.includes(id)));
      el.firstChild.textContent = slotTitle(id);
    });
    return el;
  }

  function slots(wheel) {
    const parts = [];
    for (const group of GROUPS) {
      const ids = SLOTS[wheel].filter((s) => split(s)[0] === group).map((s) => split(s)[1]);
      if (!ids.length) continue;
      const caption = h('div', { class: 'wheel-group', text: t('wheel.group.' + group) });
      reg(() => { caption.textContent = t('wheel.group.' + group); });
      parts.push(caption, h('div', { class: 'wheel-picks', role: 'group', 'aria-label': t('wheel.group.' + group) }, ids.map((id) => pick(wheel, id))));
    }
    const count = h('div', { class: 'row-desc', role: 'status' });
    reg(() => { count.textContent = t('wheel.shown', { n: uniqueIds(wheel).length - S.wheel[wheel].hidden.length, max: uniqueIds(wheel).length }); });
    const all = button(t('wheel.showall'), () => change('wheel.' + wheel + '.hidden', []), { small: true });
    all.classList.add('wheel-showall');
    reg(() => { all.disabled = S.wheel[wheel].hidden.length === 0; all.textContent = t('wheel.showall'); });
    return h('div', { class: 'wheel-body' }, parts, h('div', { class: 'wheel-foot' }, count, all));
  }

  function wheelCard(wheel) {
    const on = () => S.wheel[wheel].enabled;
    const body = card(
      row(t('wheel.' + wheel + '.title'), t('wheel.' + wheel + '.desc'), enableSwitch(wheel)),
      row(t('wheel.key.title'), t('wheel.key.desc'), keyChoice(wheel), { when: on }),
      row(t('wheel.slots.title'), t('wheel.slots.desc'), null, { block: true, when: on }));
    const picks = slots(wheel);
    reg(() => picks.classList.toggle('dim', !on()));
    body.append(picks);
    return [body];
  }

  (window.__plugins = window.__plugins || []).push({
    id: 'wheel',
    icon: 'chart.pie.fill',
    en: {
      'nav.wheel': 'Drag wheel', 'page.wheel': 'Drag wheel',
      'page.wheel.sub': 'Drag files from Finder, tap a key, and drop them on a slot to convert, compress or send them.',
      'wheel.enable.title': 'Use the drag wheel',
      'wheel.enable.desc': 'While you drag files or folders, tapping a key opens a wheel of slots around the pointer. Drop the files on a slot to run it. The wheel stays open until you let go of the mouse button; dropping in the middle or outside the wheel does nothing. Everything runs on this Mac, and the new files are written next to the originals, which are never changed.',
      'wheel.formats.title': 'Convert formats', 'wheel.formats.desc': 'Convert the files to another format: images, video, audio, PDF and documents, and any file or folder to ZIP, TAR or TGZ.',
      'wheel.tools.title': 'Tools', 'wheel.tools.desc': 'Make images, video and audio smaller, remove metadata, cut out a background, copy the text in an image, annotate it, and merge or split PDFs.',
      'wheel.actions.title': 'Actions', 'wheel.actions.desc': 'Copy the path or the files, send with AirDrop, open Terminal in the folder, show in Finder, or put the files on the shelf.',
      'wheel.key.other': 'Other key…', 'wheel.key.press': 'Press a key…',
      'wheel.key.title': 'Key', 'wheel.key.desc': 'Tap this key, on its own, while dragging, or choose Other key and press any key you like. Two wheels that are on cannot share a key. Option on its own stays a plain copy drag unless a wheel uses it, and Escape always ends the drag.',
      'wheel.slots.title': 'Slots', 'wheel.slots.desc': 'Choose which slots this wheel can show. A wheel shows only the slots that suit every file you drag. New slots in a later version show up here switched on.',
      'wheel.shown': '{n} of {max} shown', 'wheel.showall': 'Show all',
      'wheel.group.image': 'Images', 'wheel.group.video': 'Video', 'wheel.group.audio': 'Audio', 'wheel.group.pdf': 'PDF', 'wheel.group.document': 'Documents', 'wheel.group.any': 'Any file',
      'wheel.slot.compress': 'Compress', 'wheel.slot.half': 'Half size', 'wheel.slot.quarter': 'Quarter size', 'wheel.slot.strip_metadata': 'Remove metadata',
      'wheel.slot.remove_background': 'Remove background', 'wheel.slot.copy_text': 'Copy text from image', 'wheel.slot.annotate': 'Annotate',
      'wheel.slot.compress_1080p': 'Shrink to 1080p', 'wheel.slot.compress_720p': 'Shrink to 720p', 'wheel.slot.compress_540p': 'Shrink to 540p',
      'wheel.slot.pdf_merge': 'Merge PDFs', 'wheel.slot.pdf_split': 'Split into pages',
      'wheel.slot.copy_path': 'Copy path', 'wheel.slot.copy': 'Copy files', 'wheel.slot.airdrop': 'AirDrop', 'wheel.slot.terminal': 'Open Terminal here',
      'wheel.slot.reveal': 'Show in Finder', 'wheel.slot.float': 'Add to shelf',
      'fault.wheel_key_taken': 'Another wheel that is on already uses that key. Choose a different key first.',
      'fault.wheel_failed': 'Could not watch the mouse for file drags.',
    },
    vi: {
      'nav.wheel': 'Vòng kéo thả', 'page.wheel': 'Vòng kéo thả',
      'page.wheel.sub': 'Kéo tệp từ Finder, chạm một phím, rồi thả vào một ô để chuyển đổi, nén hoặc gửi chúng.',
      'wheel.enable.title': 'Dùng vòng kéo thả',
      'wheel.enable.desc': 'Khi bạn kéo tệp hoặc thư mục, chạm một phím sẽ mở một vòng các ô quanh con trỏ. Thả tệp vào một ô để chạy nó. Vòng giữ nguyên cho tới khi bạn nhả nút chuột; thả ở giữa hoặc ngoài vòng thì không có gì xảy ra. Mọi việc chạy ngay trên máy Mac này, tệp mới được ghi cạnh tệp gốc và tệp gốc không bao giờ bị thay đổi.',
      'wheel.formats.title': 'Chuyển định dạng', 'wheel.formats.desc': 'Chuyển tệp sang định dạng khác: ảnh, video, âm thanh, PDF và tài liệu, còn tệp hay thư mục bất kỳ sang ZIP, TAR hoặc TGZ.',
      'wheel.tools.title': 'Công cụ', 'wheel.tools.desc': 'Làm ảnh, video và âm thanh nhỏ lại, xóa metadata, tách nền, chép chữ trong ảnh, chú thích ảnh, và gộp hoặc tách PDF.',
      'wheel.actions.title': 'Thao tác', 'wheel.actions.desc': 'Chép đường dẫn hoặc tệp, gửi bằng AirDrop, mở Terminal tại thư mục, hiện trong Finder, hoặc đưa tệp lên khay.',
      'wheel.key.other': 'Phím khác…', 'wheel.key.press': 'Nhấn một phím…',
      'wheel.key.title': 'Phím', 'wheel.key.desc': 'Chạm riêng phím này trong lúc kéo, hoặc chọn Phím khác rồi nhấn phím bạn muốn. Hai vòng đang bật không dùng chung một phím. Phím Option đứng riêng vẫn chỉ là kéo để sao chép, trừ khi có vòng dùng nó, còn Esc luôn kết thúc việc kéo.',
      'wheel.slots.title': 'Các ô', 'wheel.slots.desc': 'Chọn những ô vòng này được phép hiện. Vòng chỉ hiện các ô phù hợp với mọi tệp bạn kéo. Ô mới ở bản sau sẽ xuất hiện ở đây ở trạng thái bật.',
      'wheel.shown': 'Hiện {n} trên {max}', 'wheel.showall': 'Hiện tất cả',
      'wheel.group.image': 'Ảnh', 'wheel.group.video': 'Video', 'wheel.group.audio': 'Âm thanh', 'wheel.group.pdf': 'PDF', 'wheel.group.document': 'Tài liệu', 'wheel.group.any': 'Mọi tệp',
      'wheel.slot.compress': 'Nén', 'wheel.slot.half': 'Một nửa kích thước', 'wheel.slot.quarter': 'Một phần tư kích thước', 'wheel.slot.strip_metadata': 'Xóa metadata',
      'wheel.slot.remove_background': 'Tách nền', 'wheel.slot.copy_text': 'Chép chữ từ ảnh', 'wheel.slot.annotate': 'Chú thích',
      'wheel.slot.compress_1080p': 'Thu nhỏ xuống 1080p', 'wheel.slot.compress_720p': 'Thu nhỏ xuống 720p', 'wheel.slot.compress_540p': 'Thu nhỏ xuống 540p',
      'wheel.slot.pdf_merge': 'Gộp PDF', 'wheel.slot.pdf_split': 'Tách thành từng trang',
      'wheel.slot.copy_path': 'Chép đường dẫn', 'wheel.slot.copy': 'Chép tệp', 'wheel.slot.airdrop': 'AirDrop', 'wheel.slot.terminal': 'Mở Terminal tại đây',
      'wheel.slot.reveal': 'Hiện trong Finder', 'wheel.slot.float': 'Đưa lên khay',
      'fault.wheel_key_taken': 'Một vòng khác đang bật đã dùng phím đó. Hãy chọn phím khác trước.',
      'fault.wheel_failed': 'Không theo dõi được chuột để nhận biết việc kéo tệp.',
    },
    build() {
      return [
        card(toggleRow('wheel.enabled', t('wheel.enable.title'), t('wheel.enable.desc'))),
        ...WHEELS.flatMap(wheelCard),
      ];
    },
    selftest({ expect, go, sent, S: state }) {
      // Every slot has a title in both languages (formats and archives are written as they are).
      for (const wheel of WHEELS) {
        for (const id of uniqueIds(wheel)) {
          if (isFormat(id)) continue;
          expect(!!I18N.en['wheel.slot.' + id] && !!I18N.vi['wheel.slot.' + id], `slot ${id} has a title in English and Vietnamese`);
        }
      }
      for (const group of GROUPS) expect(!!I18N.en['wheel.group.' + group] && !!I18N.vi['wheel.group.' + group], `group ${group} has a title in both languages`);
      for (const code of ['en', 'vi']) {
        setLang(code);
        go('wheel');
        const root = document.querySelector('#scroller');
        expect(root.querySelectorAll('.card').length === 4, `${code}: the feature card and one card for each wheel`);
        for (const wheel of WHEELS) {
          expect(root.textContent.includes(t('wheel.' + wheel + '.title')), `${code}: the ${wheel} wheel has its card`);
        }
        expect(root.querySelectorAll('.seg').length === 3, `${code}: a key choice for each wheel`);
        expect(root.querySelectorAll('.wheel-pick').length === WHEELS.reduce((n, w) => n + SLOTS[w].length, 0), `${code}: a checkbox for each slot`);
        expect(!/accessib|trợ năng/i.test(root.textContent), `${code}: no Accessibility card`);
        const pngTitle = [...root.querySelectorAll('.wheel-pick')].find((p) => p.dataset.id === 'to_png').textContent;
        expect(pngTitle === 'PNG', `${code}: a format name stays as written`);
      }
      setLang('en');
      go('wheel');
      const root = document.querySelector('#scroller');
      const cards = [...root.querySelectorAll('.card')];
      const base = JSON.parse(JSON.stringify(state.wheel));
      const reset = () => { Object.assign(state.wheel, JSON.parse(JSON.stringify(base))); runSyncs(); sent.length = 0; };
      reset();

      // The feature switch.
      const feature = [...root.querySelectorAll('.switch')].find((s) => s.getAttribute('aria-label') === t('wheel.enable.title'));
      const was = state.wheel.enabled;
      feature.click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'wheel.enabled' && m.value === !was), 'the feature switch sends its setting: ' + JSON.stringify(sent));
      feature.click();
      sent.length = 0;

      // Each wheel's switch.
      WHEELS.forEach((wheel, i) => {
        const sw = cards[i + 1].querySelector('.switch');
        const before = state.wheel[wheel].enabled;
        sw.click();
        expect(sent.some((m) => m.t === 'set' && m.path === `wheel.${wheel}.enabled` && m.value === !before), `${wheel}: the switch sends its setting`);
        sw.click();
        sent.length = 0;
      });

      // A key that no other wheel on uses is sent; one that is taken is not, and says so.
      const seg = (i) => [...cards[i + 1].querySelectorAll('.seg button')];
      seg(0)[3].click();
      expect(sent.some((m) => m.path === 'wheel.formats.key' && m.value === 'option'), 'a free key is sent: ' + JSON.stringify(sent));
      reset();
      seg(0)[1].click();
      expect(!sent.some((m) => m.path === 'wheel.formats.key'), 'a key that another wheel uses is not sent');
      expect(state.wheel.formats.key === 'shift', 'and the choice stays where it was');
      expect(document.querySelector('#toast').textContent === t('fault.wheel_key_taken'), 'and the refusal is shown');
      // A wheel that is off may pick a key that is in use; turning it on is what is refused.
      reset();
      state.wheel.formats.enabled = false;
      runSyncs();
      seg(0)[1].click();
      expect(state.wheel.formats.key === 'command', 'a wheel that is off takes any key');
      sent.length = 0;
      cards[1].querySelector('.switch').click();
      expect(!sent.some((m) => m.path === 'wheel.formats.enabled'), 'turning it on with a taken key is not sent');
      reset();

      // Any other key: the button records the next key pressed and sends its name; Escape only gives up.
      const other = (i) => cards[i + 1].querySelector('.wheel-other');
      const press = (code) => window.dispatchEvent(new KeyboardEvent('keydown', { code, bubbles: true, cancelable: true }));
      sent.length = 0;
      other(0).click();
      expect(other(0).classList.contains('recording'), 'the other key button records');
      press('Escape');
      expect(!other(0).classList.contains('recording') && !sent.some((m) => m.path === 'wheel.formats.key'), 'Escape gives up and sends nothing');
      other(0).click();
      press('ShiftLeft');
      expect(other(0).classList.contains('recording'), 'a modifier alone does not end the recording');
      press('F19');
      // What is sent goes through a short throttle; the choice itself is taken at once.
      expect(state.wheel.formats.key === 'f19', 'any other key is taken: ' + state.wheel.formats.key);
      expect(other(0).classList.contains('on') && other(0).textContent.includes('F19'), 'and the button shows it');
      expect(!other(0).classList.contains('recording'), 'and the recording ends');
      reset();

      // The slots: a checkbox hides or shows one, and Show all clears the list.
      const picks = (i) => [...cards[i + 1].querySelectorAll('.wheel-pick')];
      const half = picks(1).find((p) => p.dataset.id === 'half');
      expect(half.getAttribute('aria-pressed') === 'true', 'a slot that is not hidden is checked');
      half.click();
      expect(sent.some((m) => m.path === 'wheel.tools.hidden' && m.value.includes('half') && m.value.includes('quarter')), 'hiding a slot sends the list: ' + JSON.stringify(sent));
      expect(half.getAttribute('aria-pressed') === 'false', 'and unchecks it');
      expect(cards[2].querySelector('.wheel-foot .row-desc').textContent.includes('of'), 'the count of shown slots is there');
      sent.length = 0;
      cards[2].querySelector('.wheel-showall').click();
      // Two changes of one setting within 40 ms are sent a moment apart, so the local state is what shows.
      expect(state.wheel.tools.hidden.length === 0, 'Show all clears the list');
      expect(half.getAttribute('aria-pressed') === 'true', 'and checks every slot again');
      expect(cards[2].querySelector('.wheel-showall').disabled, 'with nothing hidden there is nothing to show');
      // A slot that two groups share is one slot.
      reset();
      const pdfs = picks(0).filter((p) => p.dataset.id === 'to_pdf');
      expect(pdfs.length === 2, 'PDF is offered under images and under documents');
      pdfs[0].click();
      expect(pdfs.every((p) => p.getAttribute('aria-pressed') === 'false'), 'and hiding it hides both');
      reset();
    },
  });
})();
