// Launcher: a tab of the notch with the apps, folders and Shortcuts you pinned.
(window.__plugins = window.__plugins || []).push({
  id: 'launcher',
  icon: 'square.grid.2x2.fill',
  en: {
    'nav.launcher': 'Launcher', 'page.launcher': 'Launcher',
    'page.launcher.sub': 'Pin apps, folders and Shortcuts to a tab of the notch and open them with one press.',
    'launcher.enable.title': 'Show the Launcher tab', 'launcher.enable.desc': 'A tile does something only when you press it. An app is found where it is now, a folder opens in the Finder, and a Shortcut is run by its name.',
    'sec.launcher.pinned': 'Pinned', 'launcher.none': 'Nothing is pinned yet.',
    'launcher.kind.app': 'App', 'launcher.kind.folder': 'Folder', 'launcher.kind.shortcut': 'Shortcut',
    'launcher.up': 'Move earlier', 'launcher.down': 'Move later', 'launcher.remove': 'Remove',
    'sec.launcher.add': 'Pin something',
    'launcher.add.app': 'Add an app...', 'launcher.add.folder': 'Add a folder...',
    'launcher.add.title': 'Add', 'launcher.add.desc': 'Up to twelve tiles fit.',
    'launcher.shortcuts.load': 'Choose a Shortcut...', 'launcher.shortcuts.add': 'Pin this Shortcut', 'launcher.shortcuts.pick': 'Shortcut to pin',
    'launcher.shortcuts.title': 'Shortcuts', 'launcher.shortcuts.desc': 'Runs a Shortcut of the Shortcuts app by its name. Your Shortcuts are read only when you ask.',
    'launcher.shortcuts.reading': 'Reading your Shortcuts...', 'launcher.shortcuts.none': 'No Shortcuts were found. Make one in the Shortcuts app first.', 'launcher.shortcuts.slow': 'The Shortcuts app did not answer in time. Try again in a moment.',
    'fault.launcher_full': 'Twelve tiles is all that fit. Remove one first.', 'fault.launcher_already': 'That is pinned already.', 'fault.launcher_unusable': 'That cannot be pinned.',
  },
  vi: {
    'nav.launcher': 'Mở nhanh', 'page.launcher': 'Mở nhanh',
    'page.launcher.sub': 'Ghim ứng dụng, thư mục và Shortcuts vào một tab của notch để mở bằng một lần bấm.',
    'launcher.enable.title': 'Hiện tab Mở nhanh', 'launcher.enable.desc': 'Ô nào chỉ làm việc khi bạn bấm vào. Ứng dụng được tìm ở chỗ nó đang nằm, thư mục mở trong Finder, còn Shortcut được chạy theo tên.',
    'sec.launcher.pinned': 'Đã ghim', 'launcher.none': 'Chưa ghim gì cả.',
    'launcher.kind.app': 'Ứng dụng', 'launcher.kind.folder': 'Thư mục', 'launcher.kind.shortcut': 'Shortcut',
    'launcher.up': 'Chuyển lên trước', 'launcher.down': 'Chuyển ra sau', 'launcher.remove': 'Bỏ ghim',
    'sec.launcher.add': 'Ghim thêm',
    'launcher.add.app': 'Thêm ứng dụng...', 'launcher.add.folder': 'Thêm thư mục...',
    'launcher.add.title': 'Thêm', 'launcher.add.desc': 'Tối đa mười hai ô.',
    'launcher.shortcuts.load': 'Chọn một Shortcut...', 'launcher.shortcuts.add': 'Ghim Shortcut này', 'launcher.shortcuts.pick': 'Shortcut cần ghim',
    'launcher.shortcuts.title': 'Shortcuts', 'launcher.shortcuts.desc': 'Chạy một Shortcut của ứng dụng Phím tắt theo tên. Danh sách chỉ được đọc khi bạn yêu cầu.',
    'launcher.shortcuts.reading': 'Đang đọc các Shortcut...', 'launcher.shortcuts.none': 'Không thấy Shortcut nào. Hãy tạo một cái trong ứng dụng Phím tắt trước.', 'launcher.shortcuts.slow': 'Ứng dụng Phím tắt chưa trả lời kịp. Hãy thử lại sau giây lát.',
    'fault.launcher_full': 'Chỉ vừa mười hai ô. Hãy bỏ bớt một ô trước.', 'fault.launcher_already': 'Mục này đã được ghim rồi.', 'fault.launcher_unusable': 'Không ghim được mục này.',
  },
  build() {
    const on = () => S.launcher.enabled;
    const when = { when: on };
    const list = h('div', { class: 'launcher-items' });
    reg(() => {
      const items = S.launcher.items;
      if (!items.length) { list.replaceChildren(h('div', { class: 'row-desc', text: t('launcher.none') })); return; }
      list.replaceChildren(...items.map((item, i) => {
        const tool = (name, glyph, label, disabled) => {
          const b = button('', () => send({ t: 'action', name: name + ':' + i }), { small: true, icon: glyph, aria: label + ': ' + item.name });
          b.disabled = disabled;
          return b;
        };
        return h('div', { class: 'launcher-item' },
          h('span', { class: 'launcher-kind', text: t('launcher.kind.' + item.kind) }),
          h('span', { class: 'launcher-name', text: item.name }),
          h('span', { class: 'launcher-tools' },
            tool('launcher_up', 'chevron.up', t('launcher.up'), i === 0),
            tool('launcher_down', 'chevron.down', t('launcher.down'), i === items.length - 1),
            tool('launcher_remove', 'xmark', t('launcher.remove'), false)));
      }));
    });

    const addApp = button(t('launcher.add.app'), () => send({ t: 'action', name: 'launcher_add_app' }), { small: true });
    const addFolder = button(t('launcher.add.folder'), () => send({ t: 'action', name: 'launcher_add_folder' }), { small: true });
    const load = button(t('launcher.shortcuts.load'), () => send({ t: 'action', name: 'launcher_shortcuts' }), { small: true });
    const pick = h('select', { class: 'pick', 'aria-label': t('launcher.shortcuts.pick') });
    const pin = button(t('launcher.shortcuts.add'), () => send({ t: 'action', name: 'launcher_add_shortcut:' + pick.selectedIndex }), { small: true });
    const status = h('div', { class: 'row-desc launcher-status' });
    reg(() => {
      const l = S.launcher;
      addApp.disabled = addFolder.disabled = pin.disabled = l.full;
      pick.replaceChildren(...l.shortcuts.map((name, i) => h('option', { value: String(i), text: name })));
      pick.style.display = pin.style.display = l.shortcuts.length ? '' : 'none';
      load.disabled = l.listing;
      status.textContent = l.listing ? t('launcher.shortcuts.reading') : l.problem === 'none' ? t('launcher.shortcuts.none') : l.problem === 'slow' ? t('launcher.shortcuts.slow') : '';
    });
    return [
      card(toggleRow('launcher.enabled', t('launcher.enable.title'), t('launcher.enable.desc'))),
      sectionTitle(t('sec.launcher.pinned')),
      card(row(null, null, list, { block: true, when: on })),
      sectionTitle(t('sec.launcher.add')),
      card(
        row(t('launcher.add.title'), t('launcher.add.desc'), h('div', { class: 'launcher-add' }, addApp, addFolder), when),
        row(t('launcher.shortcuts.title'), t('launcher.shortcuts.desc'), h('div', { class: 'launcher-add' }, load, pick, pin), when),
        row(null, null, status, { block: true, when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('launcher');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const master = page.querySelector('.switch');
    const was = S.launcher.enabled;
    master.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'launcher.enabled' && m.value === !was), 'launcher: the switch sends its setting: ' + JSON.stringify(sent));
    master.click();

    const items = [...page.querySelectorAll('.launcher-item')];
    expect(items.length === S.launcher.items.length, 'launcher: a row for each pin');
    const tools = (row) => [...row.querySelectorAll('.launcher-tools .btn')];
    expect(tools(items[0])[0].disabled && !tools(items[0])[1].disabled, 'launcher: the first pin cannot go earlier');
    expect(tools(items[2])[1].disabled, 'launcher: the last pin cannot go later');
    sent.length = 0;
    tools(items[1])[0].click();
    tools(items[1])[1].click();
    tools(items[1])[2].click();
    expect(['launcher_up:1', 'launcher_down:1', 'launcher_remove:1'].every((n) => sent.some((m) => m.t === 'action' && m.name === n)), 'launcher: a pin is moved and removed by its number: ' + JSON.stringify(sent));

    const button = (label) => [...page.querySelectorAll('.btn')].find((b) => b.textContent === label);
    sent.length = 0;
    button(t('launcher.add.app')).click();
    button(t('launcher.add.folder')).click();
    button(t('launcher.shortcuts.load')).click();
    expect(['launcher_add_app', 'launcher_add_folder', 'launcher_shortcuts'].every((n) => sent.some((m) => m.name === n)), 'launcher: the add buttons send their actions: ' + JSON.stringify(sent));
    const pick = page.querySelector('select.pick');
    expect(pick.options.length === S.launcher.shortcuts.length, 'launcher: the Shortcuts that were read are offered');
    pick.selectedIndex = 1;
    sent.length = 0;
    button(t('launcher.shortcuts.add')).click();
    expect(sent.some((m) => m.name === 'launcher_add_shortcut:1'), 'launcher: a Shortcut is pinned by its number: ' + JSON.stringify(sent));

    // A full tab takes no more.
    S.launcher.full = true; runSyncs();
    expect(button(t('launcher.add.app')).disabled && button(t('launcher.add.folder')).disabled && button(t('launcher.shortcuts.add')).disabled, 'launcher: nothing more can be added when it is full');
    S.launcher.full = false; runSyncs();
  },
});
