// Calendar, reminders and meeting alerts on the notch: the settings page.
(window.__plugins = window.__plugins || []).push({
  id: 'calendar',
  icon: '<rect x="4" y="5" width="16" height="15" rx="3"/><path d="M4 10h16M9 3v4M15 3v4"/>',
  en: {
    'nav.calendar': 'Calendar',
    'page.calendar': 'Calendar',
    'page.calendar.sub': 'Events, reminders and meeting alerts on the notch.',
    'cal.enable.title': 'Calendar on the notch',
    'cal.enable.desc': 'Shows today and tomorrow, reminders that are due, and opens the notch before a meeting.',
    'cal.sec.alerts': 'Meeting alerts',
    'cal.lead.title': 'Alert before a meeting',
    'cal.lead.desc': 'The notch opens for a few seconds this long before an event starts.',
    'cal.lead.n': '{n} min',
    'cal.sound.title': 'Play a sound',
    'cal.sound.desc': 'A short system sound with each alert.',
    'cal.sec.show': 'What to show',
    'cal.reminders.title': 'Include reminders',
    'cal.reminders.desc': 'Reminders due by tomorrow or overdue, each with a button to complete it.',
    'cal.allday.title': 'Include all-day events',
    'cal.allday.desc': 'Holidays and birthdays are listed but never raise an alert.',
    'cal.sec.access': 'Access',
    'cal.access.events': 'Calendars',
    'cal.access.events.desc': 'Needed to read your events.',
    'cal.access.reminders': 'Reminders',
    'cal.access.reminders.desc': 'Needed to list your reminders.',
    'cal.state.granted': 'Allowed',
    'cal.state.not_determined': 'Not asked yet',
    'cal.state.denied': 'Denied',
    'cal.state.restricted': 'Restricted',
    'cal.state.write_only': 'Write only',
    'cal.state.unknown': 'Unknown',
    'cal.allow': 'Allow access',
    'cal.open': 'Open System Settings',
    'cal.noprompt': 'This build has no permission text, so macOS cannot ask. Use the packaged app.',
    'cal.sec.calendars': 'Calendars to include',
    'cal.sec.lists': 'Reminder lists to include',
    'cal.calendars.empty': 'Turn the feature on and allow access to list your calendars.',
    'cal.lists.empty': 'Turn on reminders and allow access to list your reminder lists.',
    'cal.include': 'Include {name}',
    'cal.sec.note': 'Privacy',
    'cal.note.title': 'Stays on this Mac',
    'cal.note.body': 'Events and reminders are read from the system and never sent anywhere. The only change Figghost makes is marking a reminder complete when you press its button. Meeting links open only for Zoom, Google Meet, Microsoft Teams and Webex, over https.',
  },
  vi: {
    'nav.calendar': 'Lịch',
    'page.calendar': 'Lịch',
    'page.calendar.sub': 'Sự kiện, lời nhắc và báo cuộc họp trên notch.',
    'cal.enable.title': 'Lịch trên notch',
    'cal.enable.desc': 'Hiện hôm nay và ngày mai, các lời nhắc đến hạn, và mở notch trước cuộc họp.',
    'cal.sec.alerts': 'Báo cuộc họp',
    'cal.lead.title': 'Báo trước cuộc họp',
    'cal.lead.desc': 'Notch mở ra vài giây trước khi sự kiện bắt đầu một khoảng thời gian như vậy.',
    'cal.lead.n': '{n} phút',
    'cal.sound.title': 'Phát âm thanh',
    'cal.sound.desc': 'Một âm thanh ngắn của hệ thống cho mỗi lần báo.',
    'cal.sec.show': 'Hiển thị gì',
    'cal.reminders.title': 'Gồm cả lời nhắc',
    'cal.reminders.desc': 'Lời nhắc đến hạn trong ngày mai hoặc đã quá hạn, mỗi mục có nút hoàn thành.',
    'cal.allday.title': 'Gồm cả sự kiện cả ngày',
    'cal.allday.desc': 'Ngày lễ và sinh nhật được liệt kê nhưng không bao giờ báo.',
    'cal.sec.access': 'Quyền truy cập',
    'cal.access.events': 'Lịch',
    'cal.access.events.desc': 'Cần để đọc các sự kiện của bạn.',
    'cal.access.reminders': 'Lời nhắc',
    'cal.access.reminders.desc': 'Cần để liệt kê các lời nhắc của bạn.',
    'cal.state.granted': 'Đã cho phép',
    'cal.state.not_determined': 'Chưa hỏi',
    'cal.state.denied': 'Đã từ chối',
    'cal.state.restricted': 'Bị hạn chế',
    'cal.state.write_only': 'Chỉ được ghi',
    'cal.state.unknown': 'Không rõ',
    'cal.allow': 'Cho phép truy cập',
    'cal.open': 'Mở Cài đặt hệ thống',
    'cal.noprompt': 'Bản này không có nội dung xin quyền nên macOS không thể hỏi. Hãy dùng ứng dụng đã đóng gói.',
    'cal.sec.calendars': 'Lịch được dùng',
    'cal.sec.lists': 'Danh sách lời nhắc được dùng',
    'cal.calendars.empty': 'Hãy bật tính năng và cho phép truy cập để liệt kê các lịch của bạn.',
    'cal.lists.empty': 'Hãy bật lời nhắc và cho phép truy cập để liệt kê các danh sách lời nhắc.',
    'cal.include': 'Dùng {name}',
    'cal.sec.note': 'Quyền riêng tư',
    'cal.note.title': 'Ở lại trên máy này',
    'cal.note.body': 'Sự kiện và lời nhắc được đọc từ hệ thống và không gửi đi đâu. Thay đổi duy nhất của Figghost là đánh dấu hoàn thành một lời nhắc khi bạn bấm nút của nó. Liên kết cuộc họp chỉ mở cho Zoom, Google Meet, Microsoft Teams và Webex, qua https.',
  },
  build() {
    const cal = () => S.calendar;
    const on = () => cal().enabled;

    const leadSelect = () => {
      const values = [1, 2, 5, 10, 15, 30, 60];
      const el = h('select', { class: 'pick', 'aria-label': t('cal.lead.title') }, values.map((v) => h('option', { value: String(v), text: t('cal.lead.n', { n: v }) })));
      el.addEventListener('change', () => change('calendar.lead_minutes', +el.value));
      reg(() => {
        const now = String(cal().lead_minutes);
        if (![...el.options].some((o) => o.value === now)) el.append(h('option', { value: now, text: t('cal.lead.n', { n: now }) }));
        el.value = now;
      });
      return el;
    };

    const accessRow = (kind) => {
      const state = () => cal()[kind + '_access'];
      const chip = pill(() => t('cal.state.' + (state() in { granted: 1, not_determined: 1, denied: 1, restricted: 1, write_only: 1 } ? state() : 'unknown')), () => state() === 'granted');
      const btn = h('button', { class: 'btn small' });
      reg(() => {
        const s = state();
        const ask = s === 'not_determined' && cal().can_prompt;
        const open = s === 'denied' || s === 'write_only';
        btn.textContent = ask ? t('cal.allow') : t('cal.open');
        btn.style.display = ask || open ? '' : 'none';
        btn.setAttribute('aria-label', btn.textContent + ': ' + t('cal.access.' + kind));
      });
      btn.addEventListener('click', () => send({ t: 'action', name: 'calendar_access_' + kind }));
      const line = row(t('cal.access.' + kind), t('cal.access.' + kind + '.desc'), [chip, btn]);
      const note = h('div', { class: 'row-desc', role: 'status' });
      reg(() => {
        note.textContent = state() === 'not_determined' && !cal().can_prompt ? t('cal.noprompt') : '';
        note.style.display = note.textContent ? '' : 'none';
      });
      line.querySelector('.row-text').append(note);
      return line;
    };

    const calendarRow = (item) => {
      const name = item.title || item.id;
      const box = h('button', { class: 'cal-check', role: 'checkbox', 'aria-checked': String(item.included), 'aria-label': t('cal.include', { name }) });
      const line = h('div', { class: 'cal-row' + (item.included ? '' : ' off') },
        box,
        h('span', { class: 'cal-swatch', style: `background:${/^#[0-9a-f]{6}$/i.test(item.color) ? item.color : '#8e8e93'}`, 'aria-hidden': 'true' }),
        h('span', { class: 'cal-name', text: name }),
        item.source && h('span', { class: 'cal-source', text: item.source }));
      box.addEventListener('click', () => {
        const next = box.getAttribute('aria-checked') !== 'true';
        box.setAttribute('aria-checked', String(next));
        line.classList.toggle('off', !next);
        for (const list of [cal().calendars, cal().lists]) for (const c of list) if (c.id === item.id) c.included = next;
        send({ t: 'set', path: 'calendar.include', value: { id: item.id, on: next } });
      });
      return line;
    };
    const calendarList = (key, empty) => {
      const box = h('div', { class: 'cal-list' });
      let last = null;
      let mine = [];
      reg(() => {
        const data = cal()[key];
        const stamp = JSON.stringify(data) + lang;
        if (stamp === last) return;
        last = stamp;
        dropSyncs(mine);
        mine = scoped(() => box.replaceChildren(...(data.length ? data.map(calendarRow) : [h('div', { class: 'empty', text: t(empty) })])));
      });
      return box;
    };

    return [
      card(toggleRow('calendar.enabled', t('cal.enable.title'), t('cal.enable.desc'))),
      sectionTitle(t('cal.sec.alerts')),
      card(
        row(t('cal.lead.title'), t('cal.lead.desc'), leadSelect(), { when: on }),
        toggleRow('calendar.sound', t('cal.sound.title'), t('cal.sound.desc'), { when: on })),
      sectionTitle(t('cal.sec.show')),
      card(
        toggleRow('calendar.reminders', t('cal.reminders.title'), t('cal.reminders.desc'), { when: on }),
        toggleRow('calendar.all_day', t('cal.allday.title'), t('cal.allday.desc'), { when: on })),
      sectionTitle(t('cal.sec.access')),
      card(accessRow('events'), accessRow('reminders')),
      sectionTitle(t('cal.sec.calendars')),
      card(calendarList('calendars', 'cal.calendars.empty')),
      sectionTitle(t('cal.sec.lists')),
      card(calendarList('lists', 'cal.lists.empty')),
      sectionTitle(t('cal.sec.note')),
      card(infoRow(t('cal.note.title'), t('cal.note.body'))),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('calendar');
    sent.length = 0;
    const sw = [...document.querySelectorAll('.switch')][0];
    const was = S.calendar.enabled;
    sw.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'calendar.enabled' && m.value === !was), 'calendar: the switch sends its setting: ' + JSON.stringify(sent));
    sw.click();

    const lead = document.querySelector('select.pick');
    lead.value = '15';
    lead.dispatchEvent(new Event('change'));
    expect(S.calendar.lead_minutes === 15, 'calendar: the lead time is a number of minutes');

    // Permission buttons follow the state.
    sent.length = 0;
    S.calendar.events_access = 'not_determined'; S.calendar.can_prompt = true; runSyncs();
    const rows = [...document.querySelectorAll('.card .row')].filter((r) => r.querySelector('.chip'));
    expect(rows.length === 2, 'calendar: a line per permission');
    const ask = rows[0].querySelector('button');
    expect(ask.style.display !== 'none' && ask.textContent === 'Allow access', 'calendar: an unanswered permission offers to ask');
    ask.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'calendar_access_events'), 'calendar: the button asks for access: ' + JSON.stringify(sent));
    S.calendar.events_access = 'granted'; runSyncs();
    expect(rows[0].querySelector('button').style.display === 'none', 'calendar: nothing to press once allowed');
    S.calendar.reminders_access = 'denied'; runSyncs();
    expect(rows[1].querySelector('button').style.display !== 'none', 'calendar: a denied permission offers System Settings');
    S.calendar.can_prompt = false; S.calendar.events_access = 'not_determined'; runSyncs();
    expect(rows[0].querySelector('button').style.display === 'none' && rows[0].textContent.includes('permission text'), 'calendar: a build that cannot ask says so');
    S.calendar.can_prompt = true; S.calendar.events_access = 'granted'; S.calendar.reminders_access = 'granted';

    // Calendars come from the system; a checkbox sends the identifier.
    S.calendar.calendars = [
      { id: 'A-1', title: 'Work', source: 'iCloud', color: '#3478f6', included: true },
      { id: 'B-2', title: 'Home', source: 'Google', color: '#30d158', included: false },
    ];
    runSyncs();
    const boxes = [...document.querySelectorAll('.cal-check')];
    expect(boxes.length === 2, 'calendar: a checkbox per calendar');
    expect(boxes[0].getAttribute('aria-checked') === 'true' && boxes[1].getAttribute('aria-checked') === 'false', 'calendar: the boxes show what is included');
    sent.length = 0;
    boxes[0].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'calendar.include' && m.value.id === 'A-1' && m.value.on === false), 'calendar: a box sends the identifier and the choice: ' + JSON.stringify(sent));
    for (const b of boxes) expect(b.getAttribute('aria-label'), 'calendar: a checkbox has a name');
    S.calendar.calendars = [];
    runSyncs();
  },
});
