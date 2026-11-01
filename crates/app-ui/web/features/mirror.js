// Mirror and teleprompter: the camera under the notch, and the note you wrote last scrolling near the lens.
(window.__plugins = window.__plugins || []).push({
  id: 'mirror',
  icon: 'person.crop.rectangle',
  en: {
    'nav.mirror': 'Mirror', 'page.mirror': 'Mirror and teleprompter',
    'page.mirror.sub': 'See yourself before a call, and read a script near the camera while you talk to it.',
    'target.mirror:mirror': 'the mirror shortcut', 'target.mirror:prompter': 'the teleprompter shortcut',
    'mirror.enable.title': 'Use the mirror and the teleprompter', 'mirror.enable.desc': 'The camera is opened only while the mirror shows, and what it sees is drawn on the screen and kept nowhere. The teleprompter reads the note you wrote last in the Notes tab and sends nothing anywhere.',
    'sec.mirror.mirror': 'Mirror',
    'mirror.shortcut.title': 'Mirror shortcut', 'mirror.shortcut.desc': 'Shows the camera, mirrored, under the notch; drag it where you like. Press it again to close.',
    'mirror.size.title': 'Size', 'mirror.size.desc': 'The mirror is a square of this many points.', 'mirror.size.n': '{n} pt',
    'mirror.camera.title': 'Camera access', 'mirror.camera.desc': 'The mirror needs access to the camera. The first time it is asked for, macOS asks you.', 'mirror.camera.open': 'Open settings',
    'mirror.toggle': 'Show the mirror', 'mirror.toggle.desc': 'Shows or closes it now.',
    'sec.mirror.prompter': 'Teleprompter',
    'mirror.prompter.shortcut.title': 'Teleprompter shortcut', 'mirror.prompter.shortcut.desc': 'Starts and stops the strip under the menu bar. While it runs, Control, Option and Command with Space pause it, with Up and Down speed it up and slow it down, and with Left start it again.',
    'mirror.note.title': 'The script', 'mirror.note.desc': 'It reads the note written last in the Notes tab, so write or paste your script there first.', 'mirror.note.none': 'There is no note to read yet.',
    'mirror.text.title': 'Letters', 'mirror.text.desc': 'The size of the text, in points.',
    'mirror.speed.title': 'Speed', 'mirror.speed.desc': 'How fast the text rises: 1 is slow, 6 is fast.',
    'mirror.hide.title': 'Keep it out of recordings', 'mirror.hide.desc': 'Screenshots, recordings and screen sharing leave the strip out, so that the people you talk to do not see your script. Some apps may still capture it.',
    'mirror.prompter.toggle': 'Start the teleprompter', 'mirror.prompter.toggle.desc': 'Starts or stops it now.',
  },
  vi: {
    'nav.mirror': 'Gương', 'page.mirror': 'Gương và máy nhắc chữ',
    'page.mirror.sub': 'Tự nhìn mình trước cuộc gọi, và đọc kịch bản ngay cạnh camera khi đang nói.',
    'target.mirror:mirror': 'phím tắt Gương', 'target.mirror:prompter': 'phím tắt Máy nhắc chữ',
    'mirror.enable.title': 'Dùng gương và máy nhắc chữ', 'mirror.enable.desc': 'Camera chỉ mở khi gương đang hiện, và hình nó thấy chỉ vẽ lên màn hình, không lưu ở đâu. Máy nhắc chữ đọc ghi chú bạn viết gần nhất trong tab Ghi chú và không gửi gì đi đâu.',
    'sec.mirror.mirror': 'Gương',
    'mirror.shortcut.title': 'Phím tắt Gương', 'mirror.shortcut.desc': 'Hiện camera, lật gương, dưới notch; kéo đi đâu tùy ý. Nhấn lại để đóng.',
    'mirror.size.title': 'Kích thước', 'mirror.size.desc': 'Gương là một hình vuông có chừng này điểm.', 'mirror.size.n': '{n} pt',
    'mirror.camera.title': 'Quyền camera', 'mirror.camera.desc': 'Gương cần quyền dùng camera. Lần đầu được hỏi, macOS sẽ hỏi bạn.', 'mirror.camera.open': 'Mở cài đặt',
    'mirror.toggle': 'Hiện gương', 'mirror.toggle.desc': 'Hiện hoặc đóng ngay bây giờ.',
    'sec.mirror.prompter': 'Máy nhắc chữ',
    'mirror.prompter.shortcut.title': 'Phím tắt Máy nhắc chữ', 'mirror.prompter.shortcut.desc': 'Bật và tắt dải chữ dưới thanh menu. Khi đang chạy, Control, Option và Command cùng Space để tạm dừng, cùng Lên và Xuống để chạy nhanh hơn và chậm hơn, cùng Trái để bắt đầu lại.',
    'mirror.note.title': 'Kịch bản', 'mirror.note.desc': 'Nó đọc ghi chú viết gần nhất trong tab Ghi chú, nên hãy viết hoặc dán kịch bản vào đó trước.', 'mirror.note.none': 'Chưa có ghi chú nào để đọc.',
    'mirror.text.title': 'Cỡ chữ', 'mirror.text.desc': 'Cỡ chữ, tính bằng điểm.',
    'mirror.speed.title': 'Tốc độ', 'mirror.speed.desc': 'Chữ trôi nhanh thế nào: 1 là chậm, 6 là nhanh.',
    'mirror.hide.title': 'Không cho vào bản ghi', 'mirror.hide.desc': 'Ảnh chụp, bản ghi màn hình và chia sẻ màn hình bỏ dải chữ ra, để người nghe không thấy kịch bản của bạn. Một vài app vẫn có thể thu được nó.',
    'mirror.prompter.toggle': 'Bật máy nhắc chữ', 'mirror.prompter.toggle.desc': 'Bật hoặc tắt ngay bây giờ.',
  },
  build() {
    const on = () => S.mirror.enabled;
    const when = { when: on };
    const act = (name) => send({ t: 'action', name });
    const camera = pill(() => (S.mirror.camera ? t('chip.access.ok') : t('chip.access.no')), () => S.mirror.camera);
    const note = row(t('mirror.note.title'), t('mirror.note.desc'), null, when);
    const none = h('div', { class: 'row-desc', role: 'status' });
    note.querySelector('.row-text').append(none);
    reg(() => { none.textContent = S.mirror.has_note ? '' : t('mirror.note.none'); none.style.display = none.textContent ? '' : 'none'; });
    return [
      card(toggleRow('mirror.enabled', t('mirror.enable.title'), t('mirror.enable.desc'))),
      sectionTitle(t('sec.mirror.mirror')),
      card(
        row(t('mirror.shortcut.title'), t('mirror.shortcut.desc'), hotkeyButton('mirror:mirror', { allowClear: true, label: t('mirror.shortcut.title') }), when),
        selectRow({ path: 'mirror.mirror_size', title: t('mirror.size.title'), desc: t('mirror.size.desc'), when: on,
          options: () => [200, 280, 360].map((n) => ({ value: n, label: t('mirror.size.n', { n }) })) }),
        row(t('mirror.camera.title'), t('mirror.camera.desc'), h('div', { class: 'row-control' }, camera,
          button(t('mirror.camera.open'), () => act('open_camera_settings'), { small: true }))),
        row(t('mirror.toggle'), t('mirror.toggle.desc'), button(t('mirror.toggle'), () => act('mirror_toggle'), { small: true }), when)),
      sectionTitle(t('sec.mirror.prompter')),
      card(
        row(t('mirror.prompter.shortcut.title'), t('mirror.prompter.shortcut.desc'), hotkeyButton('mirror:prompter', { allowClear: true, label: t('mirror.prompter.shortcut.title') }), when),
        note,
        selectRow({ path: 'mirror.text_size', title: t('mirror.text.title'), desc: t('mirror.text.desc'), when: on,
          options: () => [28, 36, 48, 60].map((n) => ({ value: n, label: t('mirror.size.n', { n }) })) }),
        selectRow({ path: 'mirror.speed', title: t('mirror.speed.title'), desc: t('mirror.speed.desc'), when: on,
          options: () => [1, 2, 3, 4, 5, 6].map((n) => ({ value: n, label: String(n) })) }),
        toggleRow('mirror.hide_in_capture', t('mirror.hide.title'), t('mirror.hide.desc'), when),
        row(t('mirror.prompter.toggle'), t('mirror.prompter.toggle.desc'), button(t('mirror.prompter.toggle'), () => act('mirror_prompter'), { small: true }), when)),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('mirror');
    sent.length = 0;
    const first = document.querySelector('.switch');
    const was = S.mirror.enabled;
    first.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'mirror.enabled' && m.value === !was), 'mirror: the switch sends its setting: ' + JSON.stringify(sent));
    first.click();
    expect(document.querySelectorAll('.hotkey').length === 2, 'mirror: a shortcut for the mirror and one for the teleprompter');
    sent.length = 0;
    [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === t('mirror.toggle')).click();
    [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === t('mirror.prompter.toggle')).click();
    expect(sent.some((m) => m.name === 'mirror_toggle') && sent.some((m) => m.name === 'mirror_prompter'), 'mirror: both can be started from the page: ' + JSON.stringify(sent));
    sent.length = 0;
    const speed = [...document.querySelectorAll('select.pick')].find((s) => [...s.options].map((o) => o.value).join() === '1,2,3,4,5,6');
    speed.value = '5';
    speed.dispatchEvent(new Event('change'));
    expect(sent.some((m) => m.t === 'set' && m.path === 'mirror.speed' && m.value === 5), 'mirror: the speed is a number: ' + JSON.stringify(sent));
    S.mirror.has_note = false; runSyncs();
    expect(document.body.textContent.includes(t('mirror.note.none')), 'mirror: without a note the page says so');
    S.mirror.has_note = true; runSyncs();
  },
});
