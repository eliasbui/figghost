// Clipboard extras: Copy Stack, pinned boards and sensitive data protection. Three groups of cards appended to
// the Clipboard page; the state is under S.stack and every change is checked again by the app.
(function () {
  const en = {
    'stack.sec.copy': 'Copy Stack',
    'stack.copy.title': 'Copy Stack',
    'stack.copy.desc': 'Double press a modifier key to start a stack. While it runs, every copy is queued and each Command+V pastes the next item in the order it was copied. It ends when the queue is empty or at another double press.',
    'stack.key.title': 'Key to double press',
    'stack.key.desc': 'Press it twice quickly, on its own. Used with another key, or held, it does nothing here.',
    'stack.key.option': 'Option', 'stack.key.command': 'Command', 'stack.key.control': 'Control', 'stack.key.shift': 'Shift',
    'stack.status.title': 'Status',
    'stack.status.needs': 'Needs Accessibility',
    'stack.status.ready': 'Ready',
    'stack.status.running': '{n} queued',
    'stack.status.open': 'Open Accessibility',
    'stack.status.desc': 'Only Command+V is followed. Pasting from a menu or with another shortcut pastes what is on the clipboard.',

    'stack.sec.boards': 'Pinned boards',
    'stack.boards.title': 'Boards',
    'stack.boards.desc': 'A board groups pinned items and shows as a chip above the clipboard list. Option+B files the selected item in the next board, and a right click on a row offers a menu. Deleting a board keeps its items pinned.',
    'stack.boards.empty': 'No boards yet.',
    'stack.boards.items': '{n} items',
    'stack.boards.name': 'Board name',
    'stack.boards.rename': 'Rename board',
    'stack.boards.remove': 'Delete board',
    'stack.boards.new': 'New board',
    'stack.boards.add': 'Add board',
    'stack.boards.count': '{n} of {max}',
    'stack.boards.confirm.title': 'Delete this board?',
    'stack.boards.confirm.body': 'The items filed in it stay pinned and go back to the plain pinned list.',

    'stack.sec.sensitive': 'Sensitive data',
    'stack.sensitive.title': 'When a copy looks like a secret',
    'stack.sensitive.desc': 'Card numbers, API keys and tokens, private keys and the value after a password or secret label are recognised. Copies from password managers and concealed copies are always left out, whatever you choose here.',
    'stack.sensitive.off': 'Do nothing', 'stack.sensitive.mask': 'Hide, then delete', 'stack.sensitive.never': 'Never keep',
    'stack.expire.title': 'Delete hidden items after',
    'stack.expire.desc': 'Pinning a hidden item keeps it.',
    'stack.expire.min': '{n} min', 'stack.expire.hour': '{n} h', 'stack.expire.day': '1 day',
    'stack.hidden.title': 'Hidden items',
    'stack.hidden.desc': 'A hidden item shows as dots with a lock and is not searchable. Press Return or click it to reveal it, and again to paste. It is never given to AI assistants.',

    'fault.board_empty': 'Type a name for the board.',
    'fault.board_too_long': 'That name is too long.',
    'fault.board_bad_chars': 'That name has characters that cannot be used.',
    'fault.board_reserved': 'That name belongs to a filter. Pick another.',
    'fault.board_taken': 'A board with that name already exists.',
    'fault.board_full': 'The limit of boards is reached.',
    'fault.board_missing': 'That board no longer exists.',
    'fault.board_failed': 'Could not change the boards.',
  };

  const vi = {
    'stack.sec.copy': 'Copy Stack',
    'stack.copy.title': 'Copy Stack',
    'stack.copy.desc': 'Nhấn đúp một phím bổ trợ để bắt đầu một chồng. Khi đang chạy, mỗi lần sao chép được xếp vào hàng đợi và mỗi lần Command+V dán mục kế tiếp theo đúng thứ tự đã sao chép. Chồng kết thúc khi hết hàng đợi hoặc khi nhấn đúp lần nữa.',
    'stack.key.title': 'Phím nhấn đúp',
    'stack.key.desc': 'Nhấn hai lần nhanh, chỉ riêng phím đó. Dùng cùng phím khác hoặc giữ lâu thì không có tác dụng.',
    'stack.key.option': 'Option', 'stack.key.command': 'Command', 'stack.key.control': 'Control', 'stack.key.shift': 'Shift',
    'stack.status.title': 'Trạng thái',
    'stack.status.needs': 'Cần quyền Trợ năng',
    'stack.status.ready': 'Sẵn sàng',
    'stack.status.running': '{n} mục chờ',
    'stack.status.open': 'Mở Trợ năng',
    'stack.status.desc': 'Chỉ theo dõi Command+V. Dán từ menu hoặc bằng phím tắt khác sẽ dán nội dung đang có trong bộ nhớ tạm.',

    'stack.sec.boards': 'Bảng ghim',
    'stack.boards.title': 'Bảng',
    'stack.boards.desc': 'Bảng gom các mục đã ghim và hiện thành một nhãn phía trên danh sách. Option+B đưa mục đang chọn vào bảng kế tiếp, nhấn chuột phải vào một dòng để mở menu. Xóa bảng vẫn giữ các mục ở trạng thái đã ghim.',
    'stack.boards.empty': 'Chưa có bảng nào.',
    'stack.boards.items': '{n} mục',
    'stack.boards.name': 'Tên bảng',
    'stack.boards.rename': 'Đổi tên bảng',
    'stack.boards.remove': 'Xóa bảng',
    'stack.boards.new': 'Bảng mới',
    'stack.boards.add': 'Thêm bảng',
    'stack.boards.count': '{n} trên {max}',
    'stack.boards.confirm.title': 'Xóa bảng này?',
    'stack.boards.confirm.body': 'Các mục trong bảng vẫn được ghim và quay về danh sách ghim thông thường.',

    'stack.sec.sensitive': 'Dữ liệu nhạy cảm',
    'stack.sensitive.title': 'Khi nội dung sao chép giống thông tin bí mật',
    'stack.sensitive.desc': 'Nhận ra số thẻ, khóa API và mã thông báo, khóa riêng tư và giá trị sau nhãn mật khẩu hoặc bí mật. Nội dung từ trình quản lý mật khẩu và nội dung đánh dấu ẩn luôn bị bỏ qua, dù bạn chọn gì ở đây.',
    'stack.sensitive.off': 'Không làm gì', 'stack.sensitive.mask': 'Ẩn rồi xóa', 'stack.sensitive.never': 'Không lưu',
    'stack.expire.title': 'Xóa mục bị ẩn sau',
    'stack.expire.desc': 'Ghim một mục bị ẩn sẽ giữ nó lại.',
    'stack.expire.min': '{n} phút', 'stack.expire.hour': '{n} giờ', 'stack.expire.day': '1 ngày',
    'stack.hidden.title': 'Mục bị ẩn',
    'stack.hidden.desc': 'Mục bị ẩn hiện thành dấu chấm kèm ổ khóa và không tìm kiếm được. Nhấn Return hoặc bấm vào để xem, nhấn lần nữa để dán. Không bao giờ được đưa cho trợ lý AI.',

    'fault.board_empty': 'Hãy nhập tên cho bảng.',
    'fault.board_too_long': 'Tên này quá dài.',
    'fault.board_bad_chars': 'Tên chứa ký tự không dùng được.',
    'fault.board_reserved': 'Tên này đã dành cho một bộ lọc. Hãy chọn tên khác.',
    'fault.board_taken': 'Đã có một bảng cùng tên.',
    'fault.board_full': 'Đã đạt số bảng tối đa.',
    'fault.board_missing': 'Bảng này không còn nữa.',
    'fault.board_failed': 'Không thay đổi được các bảng.',
  };

  const KEYS = ['option', 'command', 'control', 'shift'];
  const CHOICES = ['off', 'mask', 'never'];
  const MINUTES = [1, 5, 10, 30, 60, 240, 1440];

  function minutesLabel(n) {
    if (n >= 1440 && n % 1440 === 0) return n === 1440 ? t('stack.expire.day') : t('stack.expire.min', { n });
    if (n >= 60 && n % 60 === 0) return t('stack.expire.hour', { n: n / 60 });
    return t('stack.expire.min', { n });
  }

  function statusPill() {
    return pill(() => {
      if (!S.stack.accessibility) return t('stack.status.needs');
      return S.stack.active ? t('stack.status.running', { n: S.stack.queued }) : t('stack.status.ready');
    }, () => S.stack.accessibility);
  }

  function copyCards() {
    const open = button(t('stack.status.open'), () => send({ t: 'action', name: 'open_accessibility' }), { small: true });
    const status = row(t('stack.status.title'), t('stack.status.desc'), [statusPill(), open]);
    reg(() => { open.style.display = S.stack.accessibility ? 'none' : ''; });
    const key = row(t('stack.key.title'), t('stack.key.desc'), segmented({
      label: t('stack.key.title'),
      options: KEYS.map((k) => ({ value: k, label: t('stack.key.' + k) })),
      value: () => S.stack.modifier,
      onPick: (v) => change('stack.modifier', v),
    }), { when: () => S.stack.enabled });
    return [
      sectionTitle(t('stack.sec.copy')),
      card(toggleRow('stack.enabled', t('stack.copy.title'), t('stack.copy.desc')), key, status),
    ];
  }

  function boardRow(board) {
    const input = h('input', { class: 'stack-input', type: 'text', value: board.name, maxlength: String(S.stack.max_name), 'aria-label': t('stack.boards.rename') + ': ' + board.name, spellcheck: 'false' });
    const commit = () => {
      const name = input.value.trim();
      if (name !== board.name) send({ t: 'action', name: 'stack_board_rename:' + board.id + ':' + name });
      // The list redraws with the new name when the app accepts it; until then the old one stays.
      input.value = board.name;
    };
    input.addEventListener('change', commit);
    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') input.blur(); });
    const remove = h('button', {
      class: 'btn icon', title: t('stack.boards.remove'), 'aria-label': t('stack.boards.remove') + ': ' + board.name,
      onclick: () => confirmDialog({ title: t('stack.boards.confirm.title'), body: t('stack.boards.confirm.body'), confirm: t('stack.boards.remove') },
        () => send({ t: 'action', name: 'stack_board_delete:' + board.id })),
    }, icon('trash'));
    return h('div', { class: 'stack-board' }, input, h('span', { class: 'stack-count', text: t('stack.boards.items', { n: board.items }) }), remove);
  }

  function addBoardRow() {
    const input = h('input', { class: 'stack-input', type: 'text', placeholder: t('stack.boards.new'), maxlength: String(S.stack.max_name), 'aria-label': t('stack.boards.new'), spellcheck: 'false' });
    const add = button(t('stack.boards.add'), () => {
      const name = input.value.trim();
      if (!name) return;
      send({ t: 'action', name: 'stack_board_add:' + name });
      input.value = '';
    }, { icon: 'plus', small: true });
    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') add.click(); });
    const count = h('span', { class: 'stack-count' });
    reg(() => {
      count.textContent = t('stack.boards.count', { n: S.stack.boards.length, max: S.stack.max_boards });
      add.disabled = S.stack.boards.length >= S.stack.max_boards;
    });
    return h('div', { class: 'row' },
      h('div', { class: 'row-text' }, input),
      h('div', { class: 'row-control' }, count, add));
  }

  function boardCards() {
    return [
      sectionTitle(t('stack.sec.boards')),
      card(
        row(t('stack.boards.title'), t('stack.boards.desc'), null),
        dynamicList(() => S.stack.boards, boardRow, t('stack.boards.empty')),
        addBoardRow()),
    ];
  }

  function expireSelect() {
    const el = h('select', { class: 'pick', 'aria-label': t('stack.expire.title') });
    el.addEventListener('change', () => change('stack.expire_minutes', +el.value));
    reg(() => {
      const current = S.stack.expire_minutes;
      const values = MINUTES.includes(current) ? MINUTES : [...MINUTES, current].sort((a, b) => a - b);
      const have = [...el.options].map((o) => +o.value).join();
      if (have !== values.join()) el.replaceChildren(...values.map((v) => h('option', { value: String(v), text: minutesLabel(v) })));
      el.value = String(current);
    });
    return el;
  }

  function sensitiveCards() {
    const choice = row(t('stack.sensitive.title'), t('stack.sensitive.desc'), segmented({
      label: t('stack.sensitive.title'),
      options: CHOICES.map((c) => ({ value: c, label: t('stack.sensitive.' + c) })),
      value: () => S.stack.sensitive,
      onPick: (v) => change('stack.sensitive', v),
    }), { block: true });
    const expire = row(t('stack.expire.title'), t('stack.expire.desc'), expireSelect(), { when: () => S.stack.sensitive === 'mask' });
    return [
      sectionTitle(t('stack.sec.sensitive')),
      card(choice, expire, row(t('stack.hidden.title'), t('stack.hidden.desc'), null)),
    ];
  }

  (window.__plugins = window.__plugins || []).push({
    id: 'stack',
    en,
    vi,
    slots: { clipboard: () => [...copyCards(), ...boardCards(), ...sensitiveCards()] },
    selftest({ expect, go, sent, S }) {
      go('clipboard');
      const titled = (text) => [...document.querySelectorAll('.card .row, .card .stack-board')].find((r) => (r.querySelector('.row-title') || {}).textContent === text);

      // Copy Stack: the switch and the key.
      sent.length = 0;
      const copy = titled(t('stack.copy.title'));
      expect(!!copy, 'the Copy Stack row is on the clipboard page');
      copy.querySelector('.switch').click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'stack.enabled' && m.value === true), 'the Copy Stack switch sends its setting: ' + JSON.stringify(sent));
      expect(!titled(t('stack.key.title')).classList.contains('disabled'), 'the key can be chosen once the switch is on');
      sent.length = 0;
      const command = [...titled(t('stack.key.title')).querySelectorAll('button')].find((b) => b.textContent === t('stack.key.command'));
      command.click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'stack.modifier' && m.value === 'command'), 'choosing a key sends it: ' + JSON.stringify(sent));
      copy.querySelector('.switch').click();
      expect(titled(t('stack.key.title')).classList.contains('disabled'), 'the key is disabled while the switch is off');

      // Boards: list, rename, delete, add.
      S.stack.boards = [{ id: 7, name: 'Work', items: 2 }];
      runSyncs();
      const board = document.querySelector('.stack-board');
      expect(!!board && board.querySelector('input').value === 'Work', 'a board is listed with its name');
      expect(board.textContent.includes(t('stack.boards.items', { n: 2 })), 'the board shows how many items it holds');
      sent.length = 0;
      const rename = board.querySelector('input');
      rename.value = 'Home';
      rename.dispatchEvent(new Event('change'));
      expect(sent.some((m) => m.t === 'action' && m.name === 'stack_board_rename:7:Home'), 'renaming sends the id and the name: ' + JSON.stringify(sent));
      expect(rename.value === 'Work', 'the old name stays until the app accepts the new one');
      sent.length = 0;
      board.querySelector('.btn.icon').click();
      const yes = document.querySelector('#modal .btn.primary');
      expect(!!yes, 'deleting a board asks first');
      if (yes) yes.click();
      expect(sent.some((m) => m.t === 'action' && m.name === 'stack_board_delete:7'), 'confirming sends the delete: ' + JSON.stringify(sent));
      sent.length = 0;
      const fresh = [...document.querySelectorAll('input.stack-input')].find((i) => i.placeholder === t('stack.boards.new'));
      fresh.value = '  Docs  ';
      [...document.querySelectorAll('.btn')].find((b) => b.textContent === t('stack.boards.add')).click();
      expect(sent.some((m) => m.t === 'action' && m.name === 'stack_board_add:Docs'), 'adding sends the trimmed name: ' + JSON.stringify(sent));
      S.stack.boards = Array.from({ length: S.stack.max_boards }, (_, i) => ({ id: i + 1, name: 'B' + i, items: 0 }));
      runSyncs();
      expect([...document.querySelectorAll('.btn')].find((b) => b.textContent === t('stack.boards.add')).disabled, 'no board can be added at the limit');
      S.stack.boards = [];
      runSyncs();

      // Sensitive data: the choice and the expiry.
      sent.length = 0;
      const sensitive = titled(t('stack.sensitive.title'));
      [...sensitive.querySelectorAll('button')].find((b) => b.textContent === t('stack.sensitive.mask')).click();
      expect(sent.some((m) => m.t === 'set' && m.path === 'stack.sensitive' && m.value === 'mask'), 'choosing to hide sends it: ' + JSON.stringify(sent));
      const expire = titled(t('stack.expire.title'));
      expect(!expire.classList.contains('disabled'), 'the expiry can be chosen while hiding');
      sent.length = 0;
      const select = expire.querySelector('select');
      select.value = '30';
      select.dispatchEvent(new Event('change'));
      expect(S.stack.expire_minutes === 30 && sent.some((m) => m.path === 'stack.expire_minutes' && m.value === 30), 'the expiry is a number of minutes: ' + JSON.stringify(sent));
      S.stack.expire_minutes = 7;
      runSyncs();
      expect([...select.options].some((o) => o.value === '7'), 'a value from the config file that is not a preset is still listed');
      [...sensitive.querySelectorAll('button')].find((b) => b.textContent === t('stack.sensitive.off')).click();
      expect(expire.classList.contains('disabled'), 'the expiry is disabled when nothing is hidden');
      S.stack.expire_minutes = 10;

      // Status follows the permission and the stack.
      S.stack.accessibility = false;
      runSyncs();
      const status = titled(t('stack.status.title'));
      expect(status.textContent.includes(t('stack.status.needs')), 'a missing permission is shown');
      S.stack.accessibility = true; S.stack.active = true; S.stack.queued = 3;
      runSyncs();
      expect(status.textContent.includes(t('stack.status.running', { n: 3 })), 'a running stack shows its queue');
      S.stack.active = false; S.stack.queued = 0;
      runSyncs();
    },
  });
})();
