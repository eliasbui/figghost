// Notes: a scratchpad in a tab of the notch, one plain text file for each note.
(window.__plugins = window.__plugins || []).push({
  id: 'notes',
  icon: 'note.text',
  en: {
    'nav.notes': 'Notes', 'page.notes': 'Notes',
    'page.notes.sub': 'Jot something down in a tab of the notch, and find it again as a plain text file.',
    'notes.enable.title': 'Show the Notes tab', 'notes.enable.desc': 'Each note is a text file in a folder of its own, so any app can open it. What you type is saved a moment after you stop, and a deleted note goes to the Trash.',
    'sec.notes.writing': 'Writing',
    'notes.size.title': 'Text size', 'notes.size.desc': 'How large the letters are in the editor.',
    'notes.size.12': 'Small', 'notes.size.14': 'Medium', 'notes.size.16': 'Large',
    'notes.new.title': 'Start a blank note each time', 'notes.new.desc': 'The tab opens on an empty note, ready for quick capture, instead of the last one you wrote.',
    'sec.notes.files': 'Where they are',
    'notes.folder.title': 'Notes folder', 'notes.folder.desc': '{count} notes. Move, back up or sync the folder like any other.', 'notes.folder.open': 'Show in Finder',
  },
  vi: {
    'nav.notes': 'Ghi chú', 'page.notes': 'Ghi chú',
    'page.notes.sub': 'Ghi nhanh vào một tab của notch, rồi tìm lại dưới dạng tệp văn bản thường.',
    'notes.enable.title': 'Hiện tab Ghi chú', 'notes.enable.desc': 'Mỗi ghi chú là một tệp văn bản trong thư mục riêng, nên ứng dụng nào cũng mở được. Chữ bạn gõ được lưu ngay sau khi bạn ngừng, và ghi chú xóa đi sẽ vào Thùng rác.',
    'sec.notes.writing': 'Khi viết',
    'notes.size.title': 'Cỡ chữ', 'notes.size.desc': 'Chữ trong ô soạn thảo to cỡ nào.',
    'notes.size.12': 'Nhỏ', 'notes.size.14': 'Vừa', 'notes.size.16': 'Lớn',
    'notes.new.title': 'Mỗi lần mở là một ghi chú trống', 'notes.new.desc': 'Tab mở ra với ghi chú trống, sẵn để ghi nhanh, thay vì ghi chú bạn viết lần trước.',
    'sec.notes.files': 'Nằm ở đâu',
    'notes.folder.title': 'Thư mục ghi chú', 'notes.folder.desc': '{count} ghi chú. Chuyển, sao lưu hay đồng bộ thư mục như mọi thư mục khác.', 'notes.folder.open': 'Hiện trong Finder',
  },
  build() {
    const on = () => S.notes.enabled;
    const when = { when: on };
    const sizes = () => [12, 14, 16].map((n) => ({ value: n, label: t('notes.size.' + n) }));
    const folder = row(t('notes.folder.title'), t('notes.folder.desc', { count: S.notes.count }), button(t('notes.folder.open'), () => send({ t: 'action', name: 'notes_folder' }), { small: true }), when);
    reg(() => { folder.querySelector('.row-desc').textContent = t('notes.folder.desc', { count: S.notes.count }); });
    return [
      card(toggleRow('notes.enabled', t('notes.enable.title'), t('notes.enable.desc'))),
      sectionTitle(t('sec.notes.writing')),
      card(
        selectRow({ path: 'notes.text_size', title: t('notes.size.title'), desc: t('notes.size.desc'), options: sizes, when: on }),
        toggleRow('notes.new_on_open', t('notes.new.title'), t('notes.new.desc'), when)),
      sectionTitle(t('sec.notes.files')),
      card(folder),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('notes');
    const page = document.querySelector('#scroller');
    const switches = [...page.querySelectorAll('.switch')];
    expect(switches.length === 2, 'notes: one switch for the tab and one for a blank note each time');
    sent.length = 0;
    const was = S.notes.enabled;
    switches[0].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'notes.enabled' && m.value === !was), 'notes: the switch sends its setting: ' + JSON.stringify(sent));
    switches[0].click();
    sent.length = 0;
    switches[1].click();
    expect(sent.some((m) => m.path === 'notes.new_on_open'), 'notes: the blank note switch sends its own path: ' + JSON.stringify(sent));
    switches[1].click();

    const pick = page.querySelector('select.pick');
    expect(pick.options.length === 3, 'notes: three sizes to choose from');
    pick.value = '16';
    pick.dispatchEvent(new Event('change'));
    expect(S.notes.text_size === 16, 'notes: the size is a number of points');
    pick.value = '14';
    pick.dispatchEvent(new Event('change'));

    sent.length = 0;
    const open = [...page.querySelectorAll('.btn')].find((b) => b.textContent === t('notes.folder.open'));
    open.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'notes_folder'), 'notes: the folder button sends its action: ' + JSON.stringify(sent));
    expect(page.textContent.includes(S.notes.count + ' notes'), 'notes: the page says how many notes there are');
  },
});
