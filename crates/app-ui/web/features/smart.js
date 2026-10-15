// Smart Select: a page of its own. Units, money and times in the selected text, and translation.
(window.__plugins = window.__plugins || []).push({
  id: 'smart',
  icon: '<path d="M4 8V6a2 2 0 0 1 2-2h2M16 4h2a2 2 0 0 1 2 2v2M20 16v2a2 2 0 0 1-2 2h-2M8 20H6a2 2 0 0 1-2-2v-2"/><path d="M9 12h6M12 9v6"/>',
  en: {
    'nav.smart': 'Smart Select', 'page.smart': 'Smart Select',
    'page.smart.sub': 'Select text anywhere and press the shortcut to see it converted: units, money and times, and a translation of other text.',
    'target.smart': 'the Smart Select shortcut',
    'smart.enable.title': 'Use Smart Select', 'smart.enable.desc': 'It reads the selected text only when you press the shortcut or choose it from the menu bar item.',
    'smart.shortcut.title': 'Shortcut', 'smart.shortcut.desc': 'Works in any app. Return copies the highlighted answer and Escape closes the window.',
    'smart.access.title': 'Reading the selection', 'smart.access.desc': 'Needs Accessibility access. An app that does not share its selection is read by copying it for a moment: your clipboard is put back as it was and the history does not record it.',
    'smart.access.open': 'Open settings',
    'smart.try.title': 'Try it', 'smart.try.desc': 'Shows the answers for "5 km" next to the pointer, without selecting anything.', 'smart.try.button': 'Try it',
    'sec.smart.internet': 'Internet',
    'smart.net.title': 'Use the internet', 'smart.net.desc': 'Converts money with the exchange rates of the day and translates other text. The rates are downloaded from open.er-api.com and nothing about you or your text is sent. Translating sends the selected text to api.mymemory.translated.net. Off by default.',
    'smart.lang.title': 'Translate into', 'smart.lang.desc': 'The language that translations go into.', 'smart.lang.auto': 'Same as the app',
    'smart.rates.title': 'Exchange rates', 'smart.rates.never': 'Not downloaded yet', 'smart.rates.on': 'Updated {date}',
    'sec.smart.show': 'What to show',
    'smart.cur.title': 'Currencies', 'smart.cur.desc': 'Amounts are converted into these. Three-letter codes separated by spaces, up to six.', 'smart.cur.label': 'Currencies',
    'smart.zones.title': 'Time zones', 'smart.zones.desc': 'A time such as 3pm PST is also shown in your own zone and in these. Names like Asia/Tokyo or UTC separated by spaces, up to six.', 'smart.zones.label': 'Time zones',
    'sec.smart.knows': 'What it understands',
    'smart.k.units': 'Units', 'smart.k.units.desc': 'Length, weight, volume, area, speed, temperature and data size, between metric and imperial.',
    'smart.k.money': 'Money', 'smart.k.money.desc': 'Amounts such as $100, 50 EUR or 5tr d, in the currencies above.',
    'smart.k.time': 'Times', 'smart.k.time.desc': 'Times such as 3pm PST, 15:30 UTC+7 or 9am in Tokyo.',
    'smart.k.words': 'Words', 'smart.k.words.desc': 'Any other text is translated when the internet is on.',
  },
  vi: {
    'nav.smart': 'Ch\u1ecdn th\u00f4ng minh', 'page.smart': 'Ch\u1ecdn th\u00f4ng minh',
    'page.smart.sub': 'Ch\u1ecdn v\u0103n b\u1ea3n \u1edf b\u1ea5t k\u1ef3 \u0111\u00e2u r\u1ed3i nh\u1ea5n ph\u00edm t\u1eaft \u0111\u1ec3 xem n\u00f3 \u0111\u01b0\u1ee3c quy \u0111\u1ed5i: \u0111\u01a1n v\u1ecb, ti\u1ec1n t\u1ec7 v\u00e0 gi\u1edd, c\u00f9ng b\u1ea3n d\u1ecbch c\u1ee7a v\u0103n b\u1ea3n kh\u00e1c.',
    'target.smart': 'ph\u00edm t\u1eaft Smart Select',
    'smart.enable.title': 'D\u00f9ng Smart Select', 'smart.enable.desc': 'Ch\u1ec9 \u0111\u1ecdc v\u0103n b\u1ea3n \u0111ang ch\u1ecdn khi b\u1ea1n nh\u1ea5n ph\u00edm t\u1eaft ho\u1eb7c ch\u1ecdn trong bi\u1ec3u t\u01b0\u1ee3ng tr\u00ean thanh menu.',
    'smart.shortcut.title': 'Ph\u00edm t\u1eaft', 'smart.shortcut.desc': 'D\u00f9ng \u1edf m\u1ecdi \u1ee9ng d\u1ee5ng. Enter sao ch\u00e9p c\u00e2u tr\u1ea3 l\u1eddi \u0111ang ch\u1ecdn, Escape \u0111\u00f3ng c\u1eeda s\u1ed5.',
    'smart.access.title': '\u0110\u1ecdc ph\u1ea7n \u0111ang ch\u1ecdn', 'smart.access.desc': 'C\u1ea7n quy\u1ec1n Tr\u1ee3 n\u0103ng. V\u1edbi \u1ee9ng d\u1ee5ng kh\u00f4ng chia s\u1ebb ph\u1ea7n \u0111ang ch\u1ecdn, v\u0103n b\u1ea3n \u0111\u01b0\u1ee3c sao ch\u00e9p trong gi\u00e2y l\u00e1t: clipboard \u0111\u01b0\u1ee3c tr\u1ea3 l\u1ea1i nh\u01b0 c\u0169 v\u00e0 l\u1ecbch s\u1eed kh\u00f4ng ghi l\u1ea1i.',
    'smart.access.open': 'M\u1edf c\u00e0i \u0111\u1eb7t',
    'smart.try.title': 'Th\u1eed ngay', 'smart.try.desc': 'Hi\u1ec7n k\u1ebft qu\u1ea3 cho "5 km" c\u1ea1nh con tr\u1ecf, kh\u00f4ng c\u1ea7n ch\u1ecdn g\u00ec.', 'smart.try.button': 'Th\u1eed ngay',
    'sec.smart.internet': 'Internet',
    'smart.net.title': 'D\u00f9ng internet', 'smart.net.desc': 'Quy \u0111\u1ed5i ti\u1ec1n theo t\u1ef7 gi\u00e1 trong ng\u00e0y v\u00e0 d\u1ecbch v\u0103n b\u1ea3n kh\u00e1c. T\u1ef7 gi\u00e1 \u0111\u01b0\u1ee3c t\u1ea3i t\u1eeb open.er-api.com v\u00e0 kh\u00f4ng g\u1eedi g\u00ec v\u1ec1 b\u1ea1n hay v\u0103n b\u1ea3n c\u1ee7a b\u1ea1n. Khi d\u1ecbch, \u0111o\u1ea1n v\u0103n b\u1ea3n \u0111ang ch\u1ecdn \u0111\u01b0\u1ee3c g\u1eedi t\u1edbi api.mymemory.translated.net. M\u1eb7c \u0111\u1ecbnh t\u1eaft.',
    'smart.lang.title': 'D\u1ecbch sang', 'smart.lang.desc': 'Ng\u00f4n ng\u1eef m\u00e0 b\u1ea3n d\u1ecbch h\u01b0\u1edbng t\u1edbi.', 'smart.lang.auto': 'Theo \u1ee9ng d\u1ee5ng',
    'smart.rates.title': 'T\u1ef7 gi\u00e1', 'smart.rates.never': 'Ch\u01b0a t\u1ea3i', 'smart.rates.on': 'C\u1eadp nh\u1eadt {date}',
    'sec.smart.show': 'Hi\u1ec3n th\u1ecb g\u00ec',
    'smart.cur.title': 'Ti\u1ec1n t\u1ec7', 'smart.cur.desc': 'S\u1ed1 ti\u1ec1n \u0111\u01b0\u1ee3c quy \u0111\u1ed5i sang c\u00e1c lo\u1ea1i n\u00e0y. M\u00e3 ba ch\u1eef c\u00e1i, c\u00e1ch nhau b\u1eb1ng d\u1ea5u c\u00e1ch, t\u1ed1i \u0111a s\u00e1u.', 'smart.cur.label': 'Ti\u1ec1n t\u1ec7',
    'smart.zones.title': 'M\u00fai gi\u1edd', 'smart.zones.desc': 'M\u1ed9t gi\u1edd nh\u01b0 3pm PST c\u0169ng \u0111\u01b0\u1ee3c hi\u1ec7n theo m\u00fai gi\u1edd c\u1ee7a b\u1ea1n v\u00e0 c\u00e1c m\u00fai gi\u1edd n\u00e0y. T\u00ean nh\u01b0 Asia/Tokyo ho\u1eb7c UTC, c\u00e1ch nhau b\u1eb1ng d\u1ea5u c\u00e1ch, t\u1ed1i \u0111a s\u00e1u.', 'smart.zones.label': 'M\u00fai gi\u1edd',
    'sec.smart.knows': 'N\u00f3 hi\u1ec3u nh\u1eefng g\u00ec',
    'smart.k.units': '\u0110\u01a1n v\u1ecb', 'smart.k.units.desc': 'Chi\u1ec1u d\u00e0i, kh\u1ed1i l\u01b0\u1ee3ng, th\u1ec3 t\u00edch, di\u1ec7n t\u00edch, t\u1ed1c \u0111\u1ed9, nhi\u1ec7t \u0111\u1ed9 v\u00e0 dung l\u01b0\u1ee3ng d\u1eef li\u1ec7u, gi\u1eefa h\u1ec7 m\u00e9t v\u00e0 h\u1ec7 Anh M\u1ef9.',
    'smart.k.money': 'Ti\u1ec1n', 'smart.k.money.desc': 'S\u1ed1 ti\u1ec1n nh\u01b0 $100, 50 EUR hay 5tr \u0111, sang c\u00e1c lo\u1ea1i ti\u1ec1n \u1edf tr\u00ean.',
    'smart.k.time': 'Gi\u1edd', 'smart.k.time.desc': 'Gi\u1edd nh\u01b0 3pm PST, 15:30 UTC+7 hay 9am in Tokyo.',
    'smart.k.words': 'V\u0103n b\u1ea3n', 'smart.k.words.desc': 'M\u1ecdi v\u0103n b\u1ea3n kh\u00e1c s\u1ebd \u0111\u01b0\u1ee3c d\u1ecbch khi b\u1eadt internet.',
  },
  build() {
    const accessPill = pill(() => (S.smart.accessibility ? t('chip.access.ok') : t('chip.access.no')), () => S.smart.accessibility);
    const language = h('select', { class: 'pick', 'aria-label': t('smart.lang.title') });
    reg(() => {
      language.replaceChildren(h('option', { value: 'auto', text: t('smart.lang.auto') }),
        ...S.smart.languages.map((l) => h('option', { value: l.code, text: l.name })));
      language.value = S.smart.translate_to;
    });
    language.addEventListener('change', () => change('smart.translate_to', language.value));
    const rates = h('span', { class: 'row-desc' });
    reg(() => {
      const when = S.smart.rates_updated;
      rates.textContent = when ? t('smart.rates.on', { date: new Date(when * 1000).toLocaleDateString(lang, { day: 'numeric', month: 'short', year: 'numeric' }) }) : t('smart.rates.never');
    });
    const listField = (key, label, placeholder) => {
      const el = h('input', { type: 'text', class: 'text-field', 'aria-label': label, placeholder, spellcheck: 'false', autocomplete: 'off', autocapitalize: 'off' });
      const parsed = () => el.value.split(/[\s,;]+/).filter(Boolean);
      reg(() => { if (document.activeElement !== el || parsed().join(' ') === S.smart[key].join(' ')) el.value = S.smart[key].join(' '); });
      el.addEventListener('change', () => change('smart.' + key, parsed()));
      return el;
    };
    return [
      card(toggleRow('smart.enabled', t('smart.enable.title'), t('smart.enable.desc'))),
      card(
        row(t('smart.shortcut.title'), t('smart.shortcut.desc'), hotkeyButton('smart', { label: t('smart.shortcut.title') }), { when: () => S.smart.enabled }),
        row(t('smart.access.title'), t('smart.access.desc'), h('div', { class: 'row-control' }, accessPill, button(t('smart.access.open'), () => send({ t: 'action', name: 'open_accessibility' }), { small: true }))),
        row(t('smart.try.title'), t('smart.try.desc'), button(t('smart.try.button'), () => send({ t: 'action', name: 'smart_try' }), { small: true }), { when: () => S.smart.enabled })),
      sectionTitle(t('sec.smart.internet')),
      card(
        toggleRow('smart.internet', t('smart.net.title'), t('smart.net.desc')),
        row(t('smart.lang.title'), t('smart.lang.desc'), language, { when: () => S.smart.internet }),
        row(t('smart.rates.title'), null, rates, { when: () => S.smart.internet })),
      sectionTitle(t('sec.smart.show')),
      card(
        row(t('smart.cur.title'), t('smart.cur.desc'), listField('currencies', t('smart.cur.label'), 'USD EUR VND'), { block: true }),
        row(t('smart.zones.title'), t('smart.zones.desc'), listField('zones', t('smart.zones.label'), 'Asia/Tokyo UTC'), { block: true })),
      sectionTitle(t('sec.smart.knows')),
      card(
        infoRow(t('smart.k.units'), t('smart.k.units.desc')),
        infoRow(t('smart.k.money'), t('smart.k.money.desc')),
        infoRow(t('smart.k.time'), t('smart.k.time.desc')),
        infoRow(t('smart.k.words'), t('smart.k.words.desc'))),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('smart');
    // The switch writes its setting.
    sent.length = 0;
    const first = document.querySelector('.switch');
    const was = S.smart.enabled;
    first.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'smart.enabled' && m.value === !was), 'the Smart Select switch sends its setting: ' + JSON.stringify(sent));
    first.click();
    // A list field sends what was typed as a list.
    sent.length = 0;
    const field = document.querySelector('input.text-field');
    field.value = 'usd, eur  jpy';
    field.dispatchEvent(new Event('change'));
    expect(sent.some((m) => m.t === 'set' && m.path === 'smart.currencies' && m.value.join() === 'usd,eur,jpy'), 'the currency list is sent as a list: ' + JSON.stringify(sent));
    // The shortcut recorder and the Try it button are there.
    expect(document.querySelectorAll('.hotkey').length === 1, 'one shortcut recorder');
    sent.length = 0;
    const tryIt = [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === t('smart.try.button'));
    expect(!!tryIt, 'a Try it button');
    tryIt.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'smart_try'), 'Try it sends its action');
    // The language choice offers every language the app knows plus "same as the app".
    const options = [...document.querySelectorAll('select.pick option')].map((o) => o.value);
    expect(options[0] === 'auto' && options.includes('vi') && options.includes('en'), 'the language list: ' + options.join());
  },
});
