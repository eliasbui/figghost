// Launcher: the apps, folders, files, web addresses and Shortcuts you pinned, in groups, in a tab of the notch and in a
// dock at an edge of a screen.
(window.__plugins = window.__plugins || []).push({
  id: 'launcher',
  icon: 'square.grid.2x2.fill',
  en: {
    'nav.launcher': 'Launcher', 'page.launcher': 'Launcher',
    'page.launcher.sub': 'Pin apps, folders, files, websites and Shortcuts in groups, and open them with one press from the notch or a dock at the edge of the screen.',
    'launcher.enable.title': 'Show the Launcher tab', 'launcher.enable.desc': 'A tile does something only when you press it. An app is found where it is now, a folder or a file opens as the Finder would, a website in your browser, and a Shortcut is run by its name. Swipe with two fingers or use the arrow keys to move between groups.',
    'sec.launcher.groups': 'Groups', 'launcher.group.title': 'Group', 'launcher.group.desc': 'Each group holds up to twelve tiles, and there can be eight groups.',
    'launcher.group.pick': 'Group to edit', 'launcher.group.n': 'Group {n}',
    'launcher.group.name': 'Name of the group', 'launcher.group.ph': 'A name, or empty for its number', 'launcher.group.rename': 'Rename',
    'launcher.group.add': 'New group', 'launcher.group.remove': 'Remove group', 'launcher.group.up': 'Move group earlier', 'launcher.group.down': 'Move group later',
    'launcher.group.confirm': 'Remove {name}?', 'launcher.group.confirm.body': 'Its tiles are unpinned. The apps, folders and files themselves are not touched.',
    'sec.launcher.pinned': 'Pinned in {name}', 'launcher.none': 'Nothing is pinned in this group yet.',
    'launcher.kind.app': 'App', 'launcher.kind.folder': 'Folder', 'launcher.kind.file': 'File', 'launcher.kind.url': 'Website', 'launcher.kind.shortcut': 'Shortcut',
    'launcher.up': 'Move earlier', 'launcher.down': 'Move later', 'launcher.remove': 'Remove',
    'sec.launcher.add': 'Pin something',
    'launcher.add.app': 'Add an app...', 'launcher.add.folder': 'Add a folder...', 'launcher.add.file': 'Add a file...',
    'launcher.add.title': 'Add', 'launcher.add.desc': 'To the group chosen above. Right-click a folder tile to see what is in it.',
    'launcher.url.title': 'Website', 'launcher.url.desc': 'An address of the web, or an email address. Other kinds of links are not pinned.',
    'launcher.url.label': 'Address to pin', 'launcher.url.ph': 'github.com or name@example.com', 'launcher.url.add': 'Pin this address',
    'launcher.shortcuts.load': 'Choose a Shortcut...', 'launcher.shortcuts.add': 'Pin this Shortcut', 'launcher.shortcuts.pick': 'Shortcut to pin',
    'launcher.shortcuts.title': 'Shortcuts', 'launcher.shortcuts.desc': 'Runs a Shortcut of the Shortcuts app by its name. Your Shortcuts are read only when you ask.',
    'launcher.shortcuts.reading': 'Reading your Shortcuts...', 'launcher.shortcuts.none': 'No Shortcuts were found. Make one in the Shortcuts app first.', 'launcher.shortcuts.slow': 'The Shortcuts app did not answer in time. Try again in a moment.',
    'sec.launcher.dock': 'Dock at the edge of the screen',
    'launcher.dock.title': 'Show the dock', 'launcher.dock.desc': 'The same groups on glass at an edge of a screen, above your windows. It never takes the keyboard from the app you are using. Off by default; turning it off takes it away.',
    'launcher.dock_edge.title': 'Edge', 'launcher.dock_edge.desc': 'The top belongs to the menu bar and the notch.',
    'launcher.dock_edge.left': 'Left', 'launcher.dock_edge.right': 'Right', 'launcher.dock_edge.bottom': 'Bottom',
    'launcher.dock_display.title': 'Screen', 'launcher.dock_display.desc': 'When this screen is not connected, the dock goes to the main one, and back when it returns.',
    'launcher.dock_display.main': 'Main screen', 'launcher.dock_display.away': '{name} (not connected)',
    'launcher.dock_group.title': 'Group shown first', 'launcher.dock_group.desc': 'Swipe with two fingers over the dock to move between groups.',
    'launcher.dock_autohide.title': 'Hide until the pointer reaches the edge', 'launcher.dock_autohide.desc': 'The dock waits off the screen and comes out when the pointer touches the edge beside it.',
    'fault.launcher_full': 'Twelve tiles is all that fit in a group. Remove one first, or use another group.', 'fault.launcher_already': 'That is pinned in this group already.', 'fault.launcher_unusable': 'That cannot be pinned.',
    'fault.launcher_groups_full': 'Eight groups is the most there can be.',
  },
  vi: {
    'nav.launcher': 'Mở nhanh', 'page.launcher': 'Mở nhanh',
    'page.launcher.sub': 'Ghim ứng dụng, thư mục, tệp, trang web và Shortcuts theo nhóm, mở bằng một lần bấm từ notch hoặc từ dock ở cạnh màn hình.',
    'launcher.enable.title': 'Hiện tab Mở nhanh', 'launcher.enable.desc': 'Ô nào chỉ làm việc khi bạn bấm vào. Ứng dụng được tìm ở chỗ nó đang nằm, thư mục và tệp mở như Finder mở, trang web mở trong trình duyệt, còn Shortcut được chạy theo tên. Vuốt hai ngón hoặc dùng phím mũi tên để chuyển nhóm.',
    'sec.launcher.groups': 'Nhóm', 'launcher.group.title': 'Nhóm', 'launcher.group.desc': 'Mỗi nhóm giữ tối đa mười hai ô, có thể có tám nhóm.',
    'launcher.group.pick': 'Nhóm đang sửa', 'launcher.group.n': 'Nhóm {n}',
    'launcher.group.name': 'Tên nhóm', 'launcher.group.ph': 'Một cái tên, hoặc để trống để hiện số', 'launcher.group.rename': 'Đổi tên',
    'launcher.group.add': 'Nhóm mới', 'launcher.group.remove': 'Xóa nhóm', 'launcher.group.up': 'Chuyển nhóm lên trước', 'launcher.group.down': 'Chuyển nhóm ra sau',
    'launcher.group.confirm': 'Xóa {name}?', 'launcher.group.confirm.body': 'Các ô trong nhóm bị bỏ ghim. Bản thân ứng dụng, thư mục và tệp không bị động tới.',
    'sec.launcher.pinned': 'Đã ghim trong {name}', 'launcher.none': 'Nhóm này chưa ghim gì cả.',
    'launcher.kind.app': 'Ứng dụng', 'launcher.kind.folder': 'Thư mục', 'launcher.kind.file': 'Tệp', 'launcher.kind.url': 'Trang web', 'launcher.kind.shortcut': 'Shortcut',
    'launcher.up': 'Chuyển lên trước', 'launcher.down': 'Chuyển ra sau', 'launcher.remove': 'Bỏ ghim',
    'sec.launcher.add': 'Ghim thêm',
    'launcher.add.app': 'Thêm ứng dụng...', 'launcher.add.folder': 'Thêm thư mục...', 'launcher.add.file': 'Thêm tệp...',
    'launcher.add.title': 'Thêm', 'launcher.add.desc': 'Vào nhóm đang chọn ở trên. Bấm chuột phải vào ô thư mục để xem bên trong có gì.',
    'launcher.url.title': 'Trang web', 'launcher.url.desc': 'Một địa chỉ web hoặc một địa chỉ email. Các loại liên kết khác không được ghim.',
    'launcher.url.label': 'Địa chỉ cần ghim', 'launcher.url.ph': 'github.com hoặc ten@example.com', 'launcher.url.add': 'Ghim địa chỉ này',
    'launcher.shortcuts.load': 'Chọn một Shortcut...', 'launcher.shortcuts.add': 'Ghim Shortcut này', 'launcher.shortcuts.pick': 'Shortcut cần ghim',
    'launcher.shortcuts.title': 'Shortcuts', 'launcher.shortcuts.desc': 'Chạy một Shortcut của ứng dụng Phím tắt theo tên. Danh sách chỉ được đọc khi bạn yêu cầu.',
    'launcher.shortcuts.reading': 'Đang đọc các Shortcut...', 'launcher.shortcuts.none': 'Không thấy Shortcut nào. Hãy tạo một cái trong ứng dụng Phím tắt trước.', 'launcher.shortcuts.slow': 'Ứng dụng Phím tắt chưa trả lời kịp. Hãy thử lại sau giây lát.',
    'sec.launcher.dock': 'Dock ở cạnh màn hình',
    'launcher.dock.title': 'Hiện dock', 'launcher.dock.desc': 'Cùng các nhóm đó, trên nền kính ở một cạnh màn hình, nằm trên các cửa sổ. Dock không bao giờ lấy bàn phím khỏi ứng dụng bạn đang dùng. Tắt theo mặc định; tắt đi là dock biến mất.',
    'launcher.dock_edge.title': 'Cạnh', 'launcher.dock_edge.desc': 'Cạnh trên dành cho thanh menu và notch.',
    'launcher.dock_edge.left': 'Trái', 'launcher.dock_edge.right': 'Phải', 'launcher.dock_edge.bottom': 'Dưới',
    'launcher.dock_display.title': 'Màn hình', 'launcher.dock_display.desc': 'Khi màn hình này không được cắm, dock sang màn hình chính, và quay lại khi nó được cắm lại.',
    'launcher.dock_display.main': 'Màn hình chính', 'launcher.dock_display.away': '{name} (chưa cắm)',
    'launcher.dock_group.title': 'Nhóm hiện đầu tiên', 'launcher.dock_group.desc': 'Vuốt hai ngón trên dock để chuyển nhóm.',
    'launcher.dock_autohide.title': 'Ẩn cho tới khi con trỏ chạm cạnh', 'launcher.dock_autohide.desc': 'Dock nằm ngoài màn hình và hiện ra khi con trỏ chạm vào cạnh bên cạnh nó.',
    'fault.launcher_full': 'Mỗi nhóm chỉ vừa mười hai ô. Hãy bỏ bớt một ô, hoặc dùng nhóm khác.', 'fault.launcher_already': 'Mục này đã được ghim trong nhóm này rồi.', 'fault.launcher_unusable': 'Không ghim được mục này.',
    'fault.launcher_groups_full': 'Chỉ có thể có tối đa tám nhóm.',
  },
  build() {
    const on = () => S.launcher.enabled || S.launcher.dock;
    const when = { when: on };
    const dockOn = () => S.launcher.dock;
    const act = (name) => send({ t: 'action', name });
    const groups = () => S.launcher.groups;
    // The group this page edits; the tab and the dock each show their own.
    let sel = 0;
    const cur = () => groups()[Math.min(sel, groups().length - 1)];
    const label = (g, i) => g.name || t('launcher.group.n', { n: i + 1 });

    // The state comes every two seconds while anything in the app changes, and a control replaced between the press
    // and the release of a click never gets the click, so a list is rebuilt only when what it shows changed.
    const pickGroup = h('select', { class: 'pick', 'aria-label': t('launcher.group.pick') });
    pickGroup.addEventListener('change', () => { sel = +pickGroup.value; runSyncs(); });
    const nameField = h('input', { type: 'text', class: 'text-field', 'aria-label': t('launcher.group.name'), placeholder: t('launcher.group.ph'), maxlength: '24', spellcheck: 'false', autocomplete: 'off' });
    const rename = () => act('launcher_group_rename:' + sel + ':' + nameField.value.trim());
    nameField.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.isComposing) rename(); });
    const renameBtn = button(t('launcher.group.rename'), rename, { small: true });
    const addGroup = button(t('launcher.group.add'), () => { sel = groups().length; act('launcher_group_add'); }, { small: true });
    const upGroup = button('', () => { act('launcher_group_up:' + sel); sel -= 1; }, { small: true, icon: 'chevron.up', aria: t('launcher.group.up') });
    const downGroup = button('', () => { act('launcher_group_down:' + sel); sel += 1; }, { small: true, icon: 'chevron.down', aria: t('launcher.group.down') });
    const removeGroup = button(t('launcher.group.remove'), () => {
      const at = sel;
      confirmDialog({ title: t('launcher.group.confirm', { name: label(cur(), at) }), body: t('launcher.group.confirm.body'), confirm: t('launcher.group.remove') }, () => {
        act('launcher_group_remove:' + at);
        sel = Math.max(0, at - 1);
      });
    }, { small: true, danger: true });
    let shownGroups = '';
    reg(() => {
      const list = groups();
      if (sel >= list.length) sel = list.length - 1;
      if (sel < 0) sel = 0;
      const key = JSON.stringify(list.map(label)) + lang;
      if (key !== shownGroups) {
        shownGroups = key;
        pickGroup.replaceChildren(...list.map((g, i) => h('option', { value: String(i), text: label(g, i) })));
      }
      pickGroup.value = String(sel);
      if (document.activeElement !== nameField) nameField.value = cur().name;
      addGroup.disabled = S.launcher.groups_full;
      removeGroup.disabled = list.length < 2;
      upGroup.disabled = sel === 0;
      downGroup.disabled = sel === list.length - 1;
    });

    const pinnedTitle = sectionTitle('');
    reg(() => { pinnedTitle.textContent = t('sec.launcher.pinned', { name: label(cur(), sel) }); });
    const list = h('div', { class: 'launcher-items' });
    let shownItems = '';
    reg(() => {
      const items = cur().items;
      const key = sel + JSON.stringify(items) + lang;
      if (key === shownItems) return;
      shownItems = key;
      if (!items.length) { list.replaceChildren(h('div', { class: 'row-desc', text: t('launcher.none') })); return; }
      list.replaceChildren(...items.map((item, i) => {
        const tool = (name, glyph, text, disabled) => {
          const b = button('', () => act(name + ':' + sel + ':' + i), { small: true, icon: glyph, aria: text + ': ' + item.name });
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

    const addApp = button(t('launcher.add.app'), () => act('launcher_add_app:' + sel), { small: true });
    const addFolder = button(t('launcher.add.folder'), () => act('launcher_add_folder:' + sel), { small: true });
    const addFile = button(t('launcher.add.file'), () => act('launcher_add_file:' + sel), { small: true });
    const urlField = h('input', { type: 'text', class: 'text-field launcher-url', 'aria-label': t('launcher.url.label'), placeholder: t('launcher.url.ph'), maxlength: '2000', spellcheck: 'false', autocomplete: 'off', autocapitalize: 'off' });
    const pinUrl = () => {
      const address = urlField.value.trim();
      if (!address) return;
      act('launcher_add_url:' + sel + ':' + address);
      urlField.value = '';
    };
    urlField.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.isComposing) pinUrl(); });
    const addUrl = button(t('launcher.url.add'), pinUrl, { small: true });
    const load = button(t('launcher.shortcuts.load'), () => act('launcher_shortcuts'), { small: true });
    const pick = h('select', { class: 'pick', 'aria-label': t('launcher.shortcuts.pick') });
    const pin = button(t('launcher.shortcuts.add'), () => act('launcher_add_shortcut:' + sel + ':' + pick.selectedIndex), { small: true });
    const status = h('div', { class: 'row-desc launcher-status' });
    let shownShortcuts = '';
    reg(() => {
      const l = S.launcher;
      addApp.disabled = addFolder.disabled = addFile.disabled = addUrl.disabled = pin.disabled = cur().full;
      const key = JSON.stringify(l.shortcuts);
      if (key !== shownShortcuts) {
        shownShortcuts = key;
        pick.replaceChildren(...l.shortcuts.map((name, i) => h('option', { value: String(i), text: name })));
      }
      pick.style.display = pin.style.display = l.shortcuts.length ? '' : 'none';
      load.disabled = l.listing;
      status.textContent = l.listing ? t('launcher.shortcuts.reading') : l.problem === 'none' ? t('launcher.shortcuts.none') : l.problem === 'slow' ? t('launcher.shortcuts.slow') : '';
    });

    const screens = () => {
      const l = S.launcher;
      const options = [{ value: '', label: t('launcher.dock_display.main') }, ...l.screens.map((name) => ({ value: name, label: name }))];
      if (l.dock_display && !l.screens.includes(l.dock_display)) options.push({ value: l.dock_display, label: t('launcher.dock_display.away', { name: l.dock_display }) });
      return options;
    };
    return [
      card(toggleRow('launcher.enabled', t('launcher.enable.title'), t('launcher.enable.desc'))),
      sectionTitle(t('sec.launcher.groups')),
      card(
        row(t('launcher.group.title'), t('launcher.group.desc'), h('div', { class: 'launcher-add' }, pickGroup, upGroup, downGroup, addGroup), when),
        row(t('launcher.group.name'), null, h('div', { class: 'launcher-add' }, nameField, renameBtn, removeGroup), when)),
      pinnedTitle,
      card(row(null, null, list, { block: true, when: on })),
      sectionTitle(t('sec.launcher.add')),
      card(
        row(t('launcher.add.title'), t('launcher.add.desc'), h('div', { class: 'launcher-add' }, addApp, addFolder, addFile), when),
        row(t('launcher.url.title'), t('launcher.url.desc'), h('div', { class: 'launcher-add' }, urlField, addUrl), when),
        row(t('launcher.shortcuts.title'), t('launcher.shortcuts.desc'), h('div', { class: 'launcher-add' }, load, pick, pin), when),
        row(null, null, status, { block: true, when: on })),
      sectionTitle(t('sec.launcher.dock')),
      card(
        toggleRow('launcher.dock', t('launcher.dock.title'), t('launcher.dock.desc')),
        segRow('launcher.dock_edge', t('launcher.dock_edge.title'), t('launcher.dock_edge.desc'), ['left', 'right', 'bottom'], { when: dockOn }),
        selectRow({ path: 'launcher.dock_display', title: t('launcher.dock_display.title'), desc: t('launcher.dock_display.desc'), options: screens, when: dockOn }),
        selectRow({ path: 'launcher.dock_group', title: t('launcher.dock_group.title'), desc: t('launcher.dock_group.desc'), options: () => groups().map((g, i) => ({ value: i, label: label(g, i) })), when: dockOn }),
        toggleRow('launcher.dock_autohide', t('launcher.dock_autohide.title'), t('launcher.dock_autohide.desc'), { when: dockOn })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('launcher');
    const page = document.querySelector('#scroller');
    const switchOf = (path) => page.querySelector('.switch[data-path="' + path + '"]');
    sent.length = 0;
    const was = S.launcher.enabled;
    switchOf('launcher.enabled').click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'launcher.enabled' && m.value === !was), 'launcher: the switch sends its setting: ' + JSON.stringify(sent));
    switchOf('launcher.enabled').click();

    const rows = () => [...page.querySelectorAll('.launcher-item')];
    expect(rows().length === S.launcher.groups[0].items.length, 'launcher: a row for each pin of the first group');
    const tools = (row) => [...row.querySelectorAll('.launcher-tools .btn')];
    expect(tools(rows()[0])[0].disabled && !tools(rows()[0])[1].disabled, 'launcher: the first pin cannot go earlier');
    expect(tools(rows()[2])[1].disabled, 'launcher: the last pin cannot go later');
    sent.length = 0;
    tools(rows()[1])[0].click();
    tools(rows()[1])[1].click();
    tools(rows()[1])[2].click();
    expect(['launcher_up:0:1', 'launcher_down:0:1', 'launcher_remove:0:1'].every((n) => sent.some((m) => m.t === 'action' && m.name === n)), 'launcher: a pin is moved and removed by its group and number: ' + JSON.stringify(sent));

    const button = (text) => [...page.querySelectorAll('.btn')].find((b) => b.textContent === text);
    sent.length = 0;
    button(t('launcher.add.app')).click();
    button(t('launcher.add.folder')).click();
    button(t('launcher.add.file')).click();
    button(t('launcher.shortcuts.load')).click();
    expect(['launcher_add_app:0', 'launcher_add_folder:0', 'launcher_add_file:0', 'launcher_shortcuts'].every((n) => sent.some((m) => m.name === n)), 'launcher: the add buttons send their actions for the group: ' + JSON.stringify(sent));
    const pick = page.querySelector('select[aria-label="' + t('launcher.shortcuts.pick') + '"]');
    expect(pick.options.length === S.launcher.shortcuts.length, 'launcher: the Shortcuts that were read are offered');
    pick.selectedIndex = 1;
    sent.length = 0;
    button(t('launcher.shortcuts.add')).click();
    expect(sent.some((m) => m.name === 'launcher_add_shortcut:0:1'), 'launcher: a Shortcut is pinned by its number: ' + JSON.stringify(sent));

    const url = page.querySelector('.launcher-url');
    url.value = '  github.com/me  ';
    sent.length = 0;
    button(t('launcher.url.add')).click();
    expect(sent.some((m) => m.name === 'launcher_add_url:0:github.com/me') && url.value === '', 'launcher: an address is pinned trimmed and the field emptied: ' + JSON.stringify(sent));
    sent.length = 0;
    button(t('launcher.url.add')).click();
    expect(!sent.length, 'launcher: an empty address is not sent');

    // The second group is edited after it is chosen.
    const groupPick = page.querySelector('select[aria-label="' + t('launcher.group.pick') + '"]');
    expect(groupPick.options.length === S.launcher.groups.length && groupPick.options[0].text === t('launcher.group.n', { n: 1 }) && groupPick.options[1].text === 'Web', 'launcher: the groups are offered by name, or by number without one');
    groupPick.value = '1';
    groupPick.dispatchEvent(new Event('change'));
    expect(rows().length === S.launcher.groups[1].items.length, 'launcher: the rows of the chosen group show');
    const name = page.querySelector('input[aria-label="' + t('launcher.group.name') + '"]');
    expect(name.value === 'Web', 'launcher: the name of the chosen group is in its field');
    name.value = ' Work ';
    sent.length = 0;
    button(t('launcher.group.rename')).click();
    button(t('launcher.add.app')).click();
    expect(sent.some((m) => m.name === 'launcher_group_rename:1:Work') && sent.some((m) => m.name === 'launcher_add_app:1'), 'launcher: the chosen group is renamed and pinned to: ' + JSON.stringify(sent));
    const rowsBefore = rows()[0];
    runSyncs();
    expect(rows()[0] === rowsBefore, 'launcher: a refresh with nothing new keeps the rows, so a click is never lost');
    sent.length = 0;
    button(t('launcher.group.add')).click();
    expect(sent.some((m) => m.name === 'launcher_group_add'), 'launcher: a group is added');
    groupPick.value = '0';
    groupPick.dispatchEvent(new Event('change'));

    // The dock has its own switch, and its choices wait for it.
    sent.length = 0;
    switchOf('launcher.dock').click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'launcher.dock' && m.value === true), 'launcher: the dock switch sends its setting');
    const edge = [...page.querySelectorAll('.seg button')].find((b) => b.textContent === t('launcher.dock_edge.bottom'));
    sent.length = 0;
    edge.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'launcher.dock_edge' && m.value === 'bottom'), 'launcher: the edge is chosen');
    const screen = page.querySelector('select[aria-label="' + t('launcher.dock_display.title') + '"]');
    expect(screen.options.length === S.launcher.screens.length + 1, 'launcher: the main screen and each connected one are offered');
    S.launcher.dock_display = 'Projector'; runSyncs();
    expect(screen.options.length === S.launcher.screens.length + 2 && screen.value === 'Projector', 'launcher: a screen that is not connected stays chosen');
    S.launcher.dock_display = ''; runSyncs();
    switchOf('launcher.dock').click();

    // A full group takes no more.
    S.launcher.groups[0].full = true; runSyncs();
    expect([t('launcher.add.app'), t('launcher.add.folder'), t('launcher.add.file'), t('launcher.url.add'), t('launcher.shortcuts.add')].every((l) => button(l).disabled), 'launcher: nothing more can be added when the group is full');
    S.launcher.groups[0].full = false; runSyncs();
  },
});
