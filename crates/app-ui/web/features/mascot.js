// The clipboard mascot: a card at the end of the Clipboard page.
(window.__plugins = window.__plugins || []).push({
  id: 'mascot',
  en: {
    'mascot.sec': 'Mascot',
    'mascot.enable.title': 'Clipboard mascot',
    'mascot.enable.desc': 'A small pixel character comes out of the notch to fetch what you copy, brings the files you add from Finder, and sees files off when you drag them out of the shelf. It carries a stand-in, never what you copied. Nothing moves while Reduce motion is on.',
    'mascot.character.title': 'Character',
    'mascot.character.desc': 'Each one reaches in its own way: silk, a tongue, a beam, a wisp or a spell. Point at the picture to see it pleased.',
    'mascot.character.spider': 'Spider',
    'mascot.character.frog': 'Frog',
    'mascot.character.ufo': 'UFO',
    'mascot.character.ghost': 'Ghost',
    'mascot.character.wizard': 'Wizard',
    'mascot.farewell.title': 'When a file leaves the shelf',
    'mascot.farewell.desc': 'Guard: it escorts the file to where you drop it and salutes. Miss: it holds on until the file is gone, then waves goodbye.',
    'mascot.farewell.guard': 'Guard',
    'mascot.farewell.regret': 'Miss',
    'mascot.try.title': 'Try it',
    'mascot.try.desc': 'Plays a copy toward the pointer, even while the mascot is off.',
    'mascot.try': 'Try',
    'mascot.aim.title': 'Where it aims',
    'mascot.aim.on': 'At the selected text, in the apps that tell where it is, and otherwise at the pointer.',
    'mascot.aim.off': 'At the pointer. With Accessibility allowed (General page) it aims at the selected text in the apps that tell where it is.',
    'mascot.finder.title': 'Add files from Finder',
    'mascot.finder.desc': 'Right-click files in Finder and choose Add to Figghost Shelf, under Services or Quick Actions. This works with the mascot off too.',
  },
  vi: {
    'mascot.sec': 'Linh vật',
    'mascot.enable.title': 'Linh vật clipboard',
    'mascot.enable.desc': 'Một nhân vật pixel nhỏ chui ra từ notch để lấy thứ bạn vừa sao chép, mang các tệp bạn thêm từ Finder lên khay, và tiễn tệp khi bạn kéo chúng ra khỏi khay. Nó chỉ mang một hình đại diện, không bao giờ là nội dung bạn đã sao chép. Khi bật Giảm chuyển động, nhân vật không xuất hiện.',
    'mascot.character.title': 'Nhân vật',
    'mascot.character.desc': 'Mỗi nhân vật lấy đồ theo một cách riêng: tơ, lưỡi, tia sáng, làn khói hoặc phép thuật. Trỏ vào hình để thấy nó vui.',
    'mascot.character.spider': 'Nhện',
    'mascot.character.frog': 'Ếch',
    'mascot.character.ufo': 'UFO',
    'mascot.character.ghost': 'Ma',
    'mascot.character.wizard': 'Phù thủy',
    'mascot.farewell.title': 'Khi tệp rời khay',
    'mascot.farewell.desc': 'Bảo vệ: nhân vật hộ tống tệp tới nơi bạn thả và chào. Tiếc nuối: nó níu lại tới khi tệp đi mất, rồi vẫy tay tạm biệt.',
    'mascot.farewell.guard': 'Bảo vệ',
    'mascot.farewell.regret': 'Tiếc nuối',
    'mascot.try.title': 'Xem thử',
    'mascot.try.desc': 'Diễn một lần sao chép về phía con trỏ, kể cả khi linh vật đang tắt.',
    'mascot.try': 'Thử',
    'mascot.aim.title': 'Nhắm vào đâu',
    'mascot.aim.on': 'Vào đoạn chữ đang chọn, ở các ứng dụng cho biết vị trí của nó; còn lại thì vào con trỏ.',
    'mascot.aim.off': 'Vào con trỏ. Khi cho phép Trợ năng (trang Chung), nó nhắm vào đoạn chữ đang chọn ở các ứng dụng cho biết vị trí của nó.',
    'mascot.finder.title': 'Thêm tệp từ Finder',
    'mascot.finder.desc': 'Bấm chuột phải vào tệp trong Finder và chọn Thêm vào khay Figghost, trong mục Dịch vụ hoặc Tác vụ nhanh. Dùng được cả khi linh vật đang tắt.',
  },
  slots: {
    clipboard() {
      const sprite = h('div', { class: 'mascot-sprite', role: 'img' });
      reg(() => {
        const c = S.mascot.character;
        sprite.style.backgroundImage = `url("${(S.mascot.sheets || {})[c] || ''}")`;
        sprite.setAttribute('aria-label', t('mascot.character.' + c));
      });
      const pick = segmented({
        label: t('mascot.character.title'),
        options: S.mascot.characters.map((c) => ({ value: c, label: t('mascot.character.' + c) })),
        value: () => S.mascot.character,
        onPick: (c) => change('mascot.character', c),
      });
      const aim = h('div', { class: 'row-desc' });
      reg(() => { aim.textContent = t(S.mascot.accessibility ? 'mascot.aim.on' : 'mascot.aim.off'); });
      return [
        sectionTitle(t('mascot.sec')),
        card(
          toggleRow('mascot.enabled', t('mascot.enable.title'), t('mascot.enable.desc')),
          row(t('mascot.character.title'), t('mascot.character.desc'), h('div', { class: 'mascot-pick' }, sprite, pick), { block: true }),
          segRow('mascot.farewell', t('mascot.farewell.title'), t('mascot.farewell.desc'), ['guard', 'regret']),
          row(t('mascot.try.title'), t('mascot.try.desc'), button(t('mascot.try'), () => send({ t: 'action', name: 'mascot_try' }), { small: true })),
          row(t('mascot.aim.title'), null, aim, { block: true }),
          infoRow(t('mascot.finder.title'), t('mascot.finder.desc'))),
      ];
    },
  },
  selftest({ expect, go, sent, S, $ }) {
    for (const code of ['en', 'vi']) {
      setLang(code);
      go('clipboard');
      const root = $('#scroller');
      const toggle = [...root.querySelectorAll('.switch')].find((b) => b.getAttribute('aria-label') === t('mascot.enable.title'));
      const pick = root.querySelector('.mascot-pick [role="radiogroup"]');
      const sprite = root.querySelector('.mascot-sprite');
      const tryButton = [...root.querySelectorAll('button')].find((b) => b.textContent === t('mascot.try'));
      expect(toggle, `${code}: the Clipboard page has the mascot switch`);
      expect(pick && pick.querySelectorAll('button').length === S.mascot.characters.length, `${code}: every character can be picked`);
      expect(sprite && /^url\("data:image\/png;base64,/.test(sprite.style.backgroundImage), `${code}: the chosen character is drawn`);
      expect(tryButton, `${code}: there is a Try button`);
      for (const c of S.mascot.characters) expect(t('mascot.character.' + c) !== 'mascot.character.' + c, `${code}: ${c} has a name`);
      if (!toggle || !pick || !tryButton || code !== 'en') continue;
      // Changes to one setting are batched for a moment, so the sending is checked once, in English.
      const before = S.mascot.enabled;
      sent.length = 0;
      toggle.click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'mascot.enabled' && m.value === !before), `mascot.enabled sends its change: ${JSON.stringify(sent)}`);
      toggle.click();
      sent.length = 0;
      pick.querySelectorAll('button')[2].click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'mascot.character' && m.value === S.mascot.characters[2]), `a character is picked: ${JSON.stringify(sent)}`);
      sent.length = 0;
      tryButton.click();
      expect(sent.some((m) => m.t === 'action' && m.name === 'mascot_try'), `Try asks the app to play: ${JSON.stringify(sent)}`);
    }
    setLang('en');
  },
});
