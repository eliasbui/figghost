// Weather: a tab of the notch with the weather now, the next hours and the next days of the chosen cities, one at a time.
(window.__plugins = window.__plugins || []).push({
  id: 'weather',
  icon: 'cloud.sun.fill',
  en: {
    'nav.weather': 'Weather', 'page.weather': 'Weather',
    'page.weather.sub': 'See the weather of your cities in a tab of the notch, one at a time, now and for the next days.',
    'weather.enable.title': 'Show the Weather tab', 'weather.enable.desc': 'The only tab that goes online. It asks Open-Meteo, which needs no account, only while the tab is open, only for the city it shows, and no more than once in ten minutes.',
    'sec.weather.city': 'Add a city',
    'weather.city.title': 'Search for a city', 'weather.city.desc': 'Only the name you search for, and then the coordinates of the cities you add, are sent. The location of this Mac is never asked for.',
    'weather.city.label': 'City name', 'weather.city.ph': 'Hanoi', 'weather.search': 'Search',
    'weather.add': 'Add', 'weather.searching': 'Searching...', 'weather.none': 'No city found with that name.',
    'weather.problem.offline': 'No connection. Try again when you are online.', 'weather.problem.service': 'The weather service did not answer. Try again in a little while.',
    'sec.weather.places': 'Your cities',
    'weather.places.desc': 'The tab opens on the first city. Flip through them with the arrows on the tab or the Left and Right keys.',
    'weather.places.none': 'No city yet. Search for one above and press Add.',
    'weather.up': 'Move up', 'weather.remove': 'Remove',
    'fault.weather_full': 'Eight cities is the most. Remove one first.', 'fault.weather_twice': 'That city is in the list already.',
    'sec.weather.units': 'Units',
    'weather.units.title': 'Units', 'weather.units.desc': 'Degrees and the speed of the wind.',
    'weather.units.metric': 'Celsius, km/h', 'weather.units.imperial': 'Fahrenheit, mph',
    'sec.weather.sound': 'Sound', 'weather.sound.title': 'Weather sounds', 'weather.sound.desc': 'Rain, wind and thunder play softly behind the tab while it is open, matching the weather. Off by default.',
  },
  vi: {
    'nav.weather': 'Thời tiết', 'page.weather': 'Thời tiết',
    'page.weather.sub': 'Xem thời tiết của các thành phố trong tab của notch, hiện tại và vài ngày tới.',
    'weather.enable.title': 'Hiện tab Thời tiết', 'weather.enable.desc': 'Tab duy nhất có kết nối mạng. Nó hỏi Open-Meteo, không cần tài khoản, chỉ khi tab đang mở, chỉ cho thành phố đang hiện, và không quá một lần trong mười phút.',
    'sec.weather.city': 'Thêm thành phố',
    'weather.city.title': 'Tìm thành phố', 'weather.city.desc': 'Chỉ tên bạn tìm, rồi tọa độ của các thành phố bạn thêm, được gửi đi. Vị trí của máy Mac này không bao giờ được hỏi.',
    'weather.city.label': 'Tên thành phố', 'weather.city.ph': 'Hà Nội', 'weather.search': 'Tìm',
    'weather.add': 'Thêm', 'weather.searching': 'Đang tìm...', 'weather.none': 'Không tìm thấy thành phố nào có tên đó.',
    'weather.problem.offline': 'Không có kết nối. Hãy thử lại khi có mạng.', 'weather.problem.service': 'Dịch vụ thời tiết không trả lời. Hãy thử lại sau ít phút.',
    'sec.weather.places': 'Thành phố của bạn',
    'weather.places.desc': 'Tab mở ở thành phố đầu tiên. Chuyển qua lại bằng mũi tên trên tab hoặc phím Trái và Phải.',
    'weather.places.none': 'Chưa có thành phố nào. Hãy tìm ở trên rồi bấm Thêm.',
    'weather.up': 'Đưa lên', 'weather.remove': 'Bỏ',
    'fault.weather_full': 'Tối đa tám thành phố. Hãy bỏ bớt một thành phố trước.', 'fault.weather_twice': 'Thành phố này đã có trong danh sách.',
    'sec.weather.units': 'Đơn vị',
    'weather.units.title': 'Đơn vị', 'weather.units.desc': 'Độ và tốc độ gió.',
    'weather.units.metric': 'Độ C, km/h', 'weather.units.imperial': 'Độ F, mph',
    'sec.weather.sound': 'Âm thanh', 'weather.sound.title': 'Âm thanh thời tiết', 'weather.sound.desc': 'Tiếng mưa, gió và sấm nhỏ nhẹ phát phía sau tab khi tab đang mở, theo đúng thời tiết. Tắt mặc định.',
  },
  build() {
    const on = () => S.weather.enabled;
    let asked = false;
    const field = h('input', { type: 'text', class: 'text-field', 'aria-label': t('weather.city.label'), placeholder: t('weather.city.ph'), maxlength: '60', spellcheck: 'false', autocomplete: 'off' });
    const search = () => {
      const query = field.value.trim();
      if (query.length < 2) return;
      asked = true;
      send({ t: 'action', name: 'weather_search:' + query });
    };
    field.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.isComposing) search(); });
    const results = h('div', { class: 'weather-results' });
    const status = h('div', { class: 'row-desc weather-status' });
    const list = h('div', { class: 'weather-places' });
    // The state comes every two seconds while anything in the app changes, and a button replaced between the press and
    // the release of a click never gets the click, so a list is rebuilt only when what it shows changed.
    let shownFound = '';
    let shownPlaces = '';
    reg(() => {
      const w = S.weather;
      const found = JSON.stringify([w.found, w.full]);
      if (found !== shownFound) {
        shownFound = found;
        results.replaceChildren(...w.found.map((p, i) => {
          const add = button(t('weather.add'), () => send({ t: 'action', name: 'weather_add:' + i }), { small: true, aria: t('weather.add') + ': ' + p.label });
          add.disabled = w.full;
          return h('div', { class: 'weather-result' }, h('span', { class: 'weather-label', text: p.label }), add);
        }));
      }
      const places = JSON.stringify(w.places);
      if (places !== shownPlaces) {
        shownPlaces = places;
        if (!w.places.length) list.replaceChildren(h('div', { class: 'row-desc', text: t('weather.places.none') }));
        else list.replaceChildren(...w.places.map((p, i) => {
          const tool = (verb, glyph, label, disabled) => {
            const b = button('', () => send({ t: 'action', name: 'weather_' + verb + ':' + i }), { small: true, icon: glyph, aria: label + ': ' + p.label });
            b.disabled = disabled;
            return b;
          };
          return h('div', { class: 'weather-place' },
            h('span', { class: 'weather-number', text: String(i + 1) }),
            h('span', { class: 'weather-label', text: p.label }),
            h('span', { class: 'weather-tools' }, tool('up', 'chevron.up', t('weather.up'), i === 0), tool('remove', 'xmark', t('weather.remove'), false)));
        }));
      }
      status.textContent = w.searching ? t('weather.searching') : w.problem ? t('weather.problem.' + w.problem) : asked && !w.found.length ? t('weather.none') : '';
    });
    const units = () => ['metric', 'imperial'].map((u) => ({ value: u, label: t('weather.units.' + u) }));
    return [
      card(toggleRow('weather.enabled', t('weather.enable.title'), t('weather.enable.desc'))),
      sectionTitle(t('sec.weather.places')),
      card(row(null, t('weather.places.desc'), list, { block: true, when: on })),
      sectionTitle(t('sec.weather.city')),
      card(
        row(t('weather.city.title'), t('weather.city.desc'), h('div', { class: 'weather-search' }, field, button(t('weather.search'), search, { small: true })), { block: true, when: on }),
        row(null, null, h('div', { class: 'weather-found' }, results, status), { block: true, when: on })),
      sectionTitle(t('sec.weather.units')),
      card(selectRow({ path: 'weather.units', title: t('weather.units.title'), desc: t('weather.units.desc'), options: units, when: on })),
      sectionTitle(t('sec.weather.sound')),
      card(toggleRow('weather.sound', t('weather.sound.title'), t('weather.sound.desc'), { when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('weather');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const master = page.querySelector('.switch');
    const was = S.weather.enabled;
    master.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'weather.enabled' && m.value === !was), 'weather: the switch sends its setting: ' + JSON.stringify(sent));
    master.click();

    // A search sends the name, and only a name of two letters or more.
    sent.length = 0;
    const field = page.querySelector('input.text-field');
    const search = [...page.querySelectorAll('.btn')].find((b) => b.textContent === t('weather.search'));
    field.value = 'H';
    search.click();
    expect(sent.length === 0, 'weather: a name of one letter is not searched for');
    field.value = '  Hue  ';
    search.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'weather_search:Hue'), 'weather: the search sends the trimmed name: ' + JSON.stringify(sent));
    field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    expect(sent.filter((m) => m.name === 'weather_search:Hue').length === 2, 'weather: Return searches too');
    field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', isComposing: true }));
    expect(sent.filter((m) => m.name === 'weather_search:Hue').length === 2, 'weather: the Return that ends a word of an input method does not search');

    // The places found are listed, each with a button that adds it by number.
    const adds = [...page.querySelectorAll('.weather-result .btn')];
    expect(adds.length === S.weather.found.length && adds.length > 0, 'weather: a button for each place found');
    expect(adds[0].textContent === t('weather.add'), 'weather: the button says Add');
    sent.length = 0;
    adds[1].click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'weather_add:1'), 'weather: a place is added by its number: ' + JSON.stringify(sent));

    // The state comes again with nothing new: the buttons must be the same ones, or a click in progress is lost.
    S.weather = JSON.parse(JSON.stringify(S.weather)); runSyncs();
    expect(page.querySelector('.weather-result .btn') === adds[0], 'weather: an unchanged state does not replace the buttons');

    // The chosen cities, each moved up or removed by its number; the first cannot move up.
    const rows = [...page.querySelectorAll('.weather-place')];
    expect(rows.length === S.weather.places.length, 'weather: a row for each city');
    expect(rows.every((r, i) => r.textContent.includes(S.weather.places[i].label)), 'weather: the rows name the cities in order');
    const tools = (row) => [...row.querySelectorAll('.weather-tools .btn')];
    expect(tools(rows[0])[0].disabled && !tools(rows[1])[0].disabled, 'weather: the first city cannot move up');
    sent.length = 0;
    tools(rows[1])[0].click();
    tools(rows[2])[1].click();
    expect(['weather_up:1', 'weather_remove:2'].every((n) => sent.some((m) => m.t === 'action' && m.name === n)), 'weather: a city is moved up and removed by its number: ' + JSON.stringify(sent));

    // A full list takes no more, and an empty one says how to add a city.
    S.weather.full = true; runSyncs();
    expect([...page.querySelectorAll('.weather-result .btn')].every((b) => b.disabled), 'weather: nothing more can be added when the list is full');
    S.weather.full = false;
    const keep = S.weather.places;
    S.weather.places = []; runSyncs();
    expect(page.textContent.includes(t('weather.places.none')), 'weather: an empty list says how to add a city');
    S.weather.places = keep; runSyncs();

    const pick = page.querySelector('select.pick');
    expect(pick.options.length === 2, 'weather: two systems of units to choose from');
    pick.value = 'imperial';
    pick.dispatchEvent(new Event('change'));
    expect(S.weather.units === 'imperial', 'weather: the units are a word');
    pick.value = 'metric';
    pick.dispatchEvent(new Event('change'));

    sent.length = 0;
    const sound = page.querySelectorAll('.switch')[1];
    const soundWas = S.weather.sound;
    sound.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'weather.sound' && m.value === !soundWas), 'weather: the sound switch sends its setting: ' + JSON.stringify(sent));
    sound.click();
  },
});
