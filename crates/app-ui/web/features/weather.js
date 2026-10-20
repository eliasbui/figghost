// Weather: a tab of the notch with the weather now, the next hours and the next days of one city.
(window.__plugins = window.__plugins || []).push({
  id: 'weather',
  icon: 'cloud.sun.fill',
  en: {
    'nav.weather': 'Weather', 'page.weather': 'Weather',
    'page.weather.sub': 'See the weather of a city in a tab of the notch, now and for the next days.',
    'weather.enable.title': 'Show the Weather tab', 'weather.enable.desc': 'The only tab that goes online. It asks Open-Meteo, which needs no account, only while the tab is open, and no more than once in ten minutes.',
    'sec.weather.city': 'City',
    'weather.city.title': 'Search for a city', 'weather.city.desc': 'Only the name you search for, and then the coordinates of the city you choose, are sent. The location of this Mac is never asked for.',
    'weather.city.label': 'City name', 'weather.city.ph': 'Hanoi', 'weather.search': 'Search',
    'weather.place.is': 'Showing {place}.', 'weather.place.none': 'No city chosen yet.',
    'weather.use': 'Use', 'weather.searching': 'Searching...', 'weather.none': 'No city found with that name.',
    'weather.problem.offline': 'No connection. Try again when you are online.', 'weather.problem.service': 'The weather service did not answer. Try again in a little while.',
    'sec.weather.units': 'Units',
    'weather.units.title': 'Units', 'weather.units.desc': 'Degrees and the speed of the wind.',
    'weather.units.metric': 'Celsius, km/h', 'weather.units.imperial': 'Fahrenheit, mph',
  },
  vi: {
    'nav.weather': 'Thời tiết', 'page.weather': 'Thời tiết',
    'page.weather.sub': 'Xem thời tiết của một thành phố trong tab của notch, hiện tại và vài ngày tới.',
    'weather.enable.title': 'Hiện tab Thời tiết', 'weather.enable.desc': 'Tab duy nhất có kết nối mạng. Nó hỏi Open-Meteo, không cần tài khoản, chỉ khi tab đang mở và không quá một lần trong mười phút.',
    'sec.weather.city': 'Thành phố',
    'weather.city.title': 'Tìm thành phố', 'weather.city.desc': 'Chỉ tên bạn tìm, rồi tọa độ của thành phố bạn chọn, được gửi đi. Vị trí của máy Mac này không bao giờ được hỏi.',
    'weather.city.label': 'Tên thành phố', 'weather.city.ph': 'Hà Nội', 'weather.search': 'Tìm',
    'weather.place.is': 'Đang hiện {place}.', 'weather.place.none': 'Chưa chọn thành phố.',
    'weather.use': 'Dùng', 'weather.searching': 'Đang tìm...', 'weather.none': 'Không tìm thấy thành phố nào có tên đó.',
    'weather.problem.offline': 'Không có kết nối. Hãy thử lại khi có mạng.', 'weather.problem.service': 'Dịch vụ thời tiết không trả lời. Hãy thử lại sau ít phút.',
    'sec.weather.units': 'Đơn vị',
    'weather.units.title': 'Đơn vị', 'weather.units.desc': 'Độ và tốc độ gió.',
    'weather.units.metric': 'Độ C, km/h', 'weather.units.imperial': 'Độ F, mph',
  },
  build() {
    const on = () => S.weather.enabled;
    const when = { when: on };
    let asked = false;
    const field = h('input', { type: 'text', class: 'text-field', 'aria-label': t('weather.city.label'), placeholder: t('weather.city.ph'), maxlength: '60', spellcheck: 'false', autocomplete: 'off' });
    const search = () => {
      const query = field.value.trim();
      if (query.length < 2) return;
      asked = true;
      send({ t: 'action', name: 'weather_search:' + query });
    };
    field.addEventListener('keydown', (e) => { if (e.key === 'Enter') search(); });
    const chosen = h('div', { class: 'row-desc' });
    const results = h('div', { class: 'weather-results' });
    const status = h('div', { class: 'row-desc weather-status' });
    reg(() => {
      const w = S.weather;
      chosen.textContent = w.place ? t('weather.place.is', { place: w.place }) : t('weather.place.none');
      results.replaceChildren(...w.found.map((p, i) => h('div', { class: 'weather-result' }, h('span', { class: 'weather-label', text: p.label }), button(t('weather.use'), () => send({ t: 'action', name: 'weather_pick:' + i }), { small: true }))));
      status.textContent = w.searching ? t('weather.searching') : w.problem ? t('weather.problem.' + w.problem) : asked && !w.found.length ? t('weather.none') : '';
    });
    const units = () => ['metric', 'imperial'].map((u) => ({ value: u, label: t('weather.units.' + u) }));
    return [
      card(toggleRow('weather.enabled', t('weather.enable.title'), t('weather.enable.desc'))),
      sectionTitle(t('sec.weather.city')),
      card(
        row(t('weather.city.title'), t('weather.city.desc'), h('div', { class: 'weather-search' }, field, button(t('weather.search'), search, { small: true })), { block: true, when: on }),
        row(null, null, h('div', { class: 'weather-found' }, chosen, results, status), { block: true, when: on })),
      sectionTitle(t('sec.weather.units')),
      card(selectRow({ path: 'weather.units', title: t('weather.units.title'), desc: t('weather.units.desc'), options: units, when: on })),
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

    // The places found are listed, each with a button that picks it by number.
    const uses = [...page.querySelectorAll('.weather-result .btn')];
    expect(uses.length === S.weather.found.length && uses.length > 0, 'weather: a button for each place found');
    sent.length = 0;
    uses[1].click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'weather_pick:1'), 'weather: a place is picked by its number: ' + JSON.stringify(sent));
    expect(page.textContent.includes(S.weather.place), 'weather: the page says which city is shown');

    const pick = page.querySelector('select.pick');
    expect(pick.options.length === 2, 'weather: two systems of units to choose from');
    pick.value = 'imperial';
    pick.dispatchEvent(new Event('change'));
    expect(S.weather.units === 'imperial', 'weather: the units are a word');
    pick.value = 'metric';
    pick.dispatchEvent(new Event('change'));
  },
});
