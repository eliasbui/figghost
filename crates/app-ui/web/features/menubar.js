// Hiding menu bar icons.
// A card at the end of the Notch page rather than a page of its own: it is four rows and a note, the sidebar is
// already long, and its main caveat (icons beside the notch are out of reach) belongs next to the notch.
(function () {
  const DELAYS = [0, 5, 10, 15, 30, 60, 300];

  function delaySelect() {
    const label = (n) => (n === 0 ? t('menubar.auto.never') : t('menubar.auto.n', { n }));
    const el = h('select', { class: 'pick', 'aria-label': t('menubar.auto.title') }, DELAYS.map((v) => h('option', { value: String(v), text: label(v) })));
    el.addEventListener('change', () => change('menubar.auto_seconds', +el.value));
    reg(() => {
      const now = S.menubar.auto_seconds;
      // A value typed into the file by hand is shown as it is rather than snapped to the list.
      if (![...el.options].some((o) => o.value === String(now))) el.append(h('option', { value: String(now), text: label(now) }));
      el.value = String(now);
    });
    return el;
  }

  function stateRow() {
    const names = { off: 'menubar.status.off', showing: 'menubar.status.showing', hiding: 'menubar.status.hiding', measuring: 'menubar.status.measuring', unavailable: 'menubar.status.unavailable' };
    const chip = pill(() => t(names[S.menubar.status] || names.off), () => S.menubar.status === 'hiding');
    const btn = h('button', { class: 'btn small', onclick: () => send({ t: 'action', name: 'menubar_toggle' }) });
    reg(() => {
      const hiding = S.menubar.status === 'hiding';
      btn.textContent = hiding ? t('menubar.show.now') : t('menubar.hide.now');
      btn.disabled = S.menubar.status !== 'hiding' && S.menubar.status !== 'showing';
    });
    return row(t('menubar.state.title'), null, [chip, btn]);
  }

  function cards() {
    const settings = card(
      toggleRow('menubar.enabled', t('menubar.enable.title'), t('menubar.enable.desc')),
      row(t('menubar.auto.title'), t('menubar.auto.desc'), delaySelect(), { when: () => S.menubar.enabled }),
      toggleRow('menubar.click_only', t('menubar.click.title'), t('menubar.click.desc'), { when: () => S.menubar.enabled }),
      stateRow());
    return [
      sectionTitle(t('sec.menubar')),
      settings,
      card(infoRow(t('menubar.how.title'), t('menubar.how.body')), infoRow(t('menubar.limits.title'), t('menubar.limits.body'))),
    ];
  }

  (window.__plugins = window.__plugins || []).push({
    id: 'menubar',
    en: {
      'sec.menubar': 'Menu bar icons',
      'menubar.enable.title': 'Hide menu bar icons',
      'menubar.enable.desc': 'Adds an arrow and a divider to the menu bar. Icons to the left of the divider hide when you press the arrow. Off by default.',
      'menubar.auto.title': 'Hide again by itself', 'menubar.auto.desc': 'How long the icons stay after you show them.',
      'menubar.auto.never': 'Never', 'menubar.auto.n': 'After {n} seconds',
      'menubar.click.title': 'Show only when the arrow is clicked', 'menubar.click.desc': 'Off also shows the icons when the pointer rests on the arrow.',
      'menubar.state.title': 'Right now',
      'menubar.status.off': 'Off', 'menubar.status.showing': 'Icons showing', 'menubar.status.hiding': 'Icons hidden',
      'menubar.status.measuring': 'Measuring the menu bar', 'menubar.status.unavailable': 'Cannot hide on this screen',
      'menubar.hide.now': 'Hide now', 'menubar.show.now': 'Show now',
      'menubar.how.title': 'Choosing the icons to hide',
      'menubar.how.body': 'Hold Command and drag an icon to the left of the divider (the thin line) to hide it, or to the right of it to keep it visible. A newly started app puts its icon at the far left, which is the hidden side: drag it over once. If the arrow ever ends up on the hidden side, "Show hidden menu bar icons" in the Fig menu still works.',
      'menubar.limits.title': 'What this cannot do',
      'menubar.limits.body': 'Icons that do not fit beside the notch are hidden by macOS itself and cannot be shown by this. On some macOS versions the longest divider that works depends on the display and on the app in front, so Figghost measures it and checks again after every change; where none works it leaves everything showing and says so here. Turning this off or quitting Figghost removes the arrow and the divider and every icon returns.',
    },
    vi: {
      'sec.menubar': 'Biểu tượng trên thanh menu',
      'menubar.enable.title': 'Ẩn biểu tượng trên thanh menu',
      'menubar.enable.desc': 'Thêm một mũi tên và một vạch ngăn vào thanh menu. Các biểu tượng nằm bên trái vạch ngăn sẽ ẩn khi bạn bấm mũi tên. Mặc định tắt.',
      'menubar.auto.title': 'Tự ẩn lại', 'menubar.auto.desc': 'Thời gian các biểu tượng được giữ lại sau khi bạn hiện chúng.',
      'menubar.auto.never': 'Không bao giờ', 'menubar.auto.n': 'Sau {n} giây',
      'menubar.click.title': 'Chỉ hiện khi bấm mũi tên', 'menubar.click.desc': 'Khi tắt, các biểu tượng cũng hiện khi con trỏ dừng trên mũi tên.',
      'menubar.state.title': 'Lúc này',
      'menubar.status.off': 'Đang tắt', 'menubar.status.showing': 'Đang hiện biểu tượng', 'menubar.status.hiding': 'Đã ẩn biểu tượng',
      'menubar.status.measuring': 'Đang đo thanh menu', 'menubar.status.unavailable': 'Không ẩn được trên màn hình này',
      'menubar.hide.now': 'Ẩn ngay', 'menubar.show.now': 'Hiện ngay',
      'menubar.how.title': 'Chọn biểu tượng cần ẩn',
      'menubar.how.body': 'Giữ phím Command và kéo một biểu tượng sang bên trái vạch ngăn (đường kẻ mảnh) để ẩn nó, hoặc sang bên phải để giữ nó luôn hiện. Ứng dụng vừa mở đặt biểu tượng ở ngoài cùng bên trái, tức là phía bị ẩn: hãy kéo nó sang một lần. Nếu mũi tên lỡ nằm ở phía bị ẩn, mục "Hiện các biểu tượng thanh menu đã ẩn" trong menu Fig vẫn dùng được.',
      'menubar.limits.title': 'Những gì không làm được',
      'menubar.limits.body': 'Các biểu tượng không còn chỗ cạnh notch sẽ do chính macOS ẩn đi và tính năng này không hiện lại được. Trên một số phiên bản macOS, độ dài lớn nhất của vạch ngăn còn dùng được phụ thuộc vào màn hình và ứng dụng đang ở phía trước, nên Figghost đo độ dài đó và kiểm tra lại sau mỗi thay đổi; nơi nào không có độ dài nào dùng được, Figghost để mọi thứ hiện và báo tại đây. Tắt tính năng hoặc thoát Figghost sẽ gỡ mũi tên và vạch ngăn, mọi biểu tượng trở lại.',
    },
    slots: { menubar: cards },
    selftest({ expect, go, sent, S: state }) {
      go('menubar');
      expect(!!document.querySelector('.pick'), 'the delay is a choice');
      const sw = [...document.querySelectorAll('.switch')].find((s) => s.getAttribute('aria-label') === t('menubar.enable.title'));
      expect(!!sw, 'the switch exists');
      sent.length = 0;
      sw.click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'menubar.enabled' && m.value === true), 'the switch sends its setting: ' + JSON.stringify(sent));
      const select = [...document.querySelectorAll('select.pick')].find((s) => s.getAttribute('aria-label') === t('menubar.auto.title'));
      expect(!!select, 'the delay choice exists');
      select.value = '30';
      select.dispatchEvent(new Event('change'));
      expect(state.menubar.auto_seconds === 30, 'the delay is a number of seconds');
      const click = [...document.querySelectorAll('.switch')].find((s) => s.getAttribute('aria-label') === t('menubar.click.title'));
      const was = state.menubar.click_only;
      click.click();
      expect(state.menubar.click_only === !was, 'click only flips');
      // The state row follows what the running feature says, and the button asks for the opposite.
      state.menubar.status = 'hiding';
      runSyncs();
      const btn = [...document.querySelectorAll('button.btn')].find((b) => b.textContent === t('menubar.show.now'));
      expect(!!btn && !btn.disabled, 'while hidden the button offers to show');
      sent.length = 0;
      btn.click();
      expect(sent.some((m) => m.t === 'action' && m.name === 'menubar_toggle'), 'and asks the app to toggle');
      state.menubar.status = 'unavailable';
      runSyncs();
      expect([...document.querySelectorAll('.chip')].some((c) => c.textContent.includes(t('menubar.status.unavailable'))), 'a bar that cannot hide says so');
      state.menubar.status = 'off';
      runSyncs();
      const off = [...document.querySelectorAll('button.btn')].find((b) => b.textContent === t('menubar.hide.now'));
      expect(!!off && off.disabled, 'with the feature off there is nothing to toggle');
      state.menubar.status = 'showing';
      state.menubar.enabled = false;
      runSyncs();
    },
  });
})();
