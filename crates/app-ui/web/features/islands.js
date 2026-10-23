// Live islands: small notices that come out of the closed notch when something happens on the Mac.
(window.__plugins = window.__plugins || []).push({
  id: 'islands',
  icon: 'bell.badge.fill',
  en: {
    'nav.islands': 'Live islands', 'page.islands': 'Live islands',
    'page.islands.sub': 'Small notices that come out of the closed notch for a moment: Caps Lock, the charger, a full or low battery, a drive, a finished download, a camera or microphone in use.',
    'islands.enable.title': 'Show live islands', 'islands.enable.desc': 'Nothing here reads what is on the screen or what is typed. Each notice can be switched off below.',
    'sec.islands.which': 'Which notices',
    'islands.caps.title': 'Caps Lock', 'islands.caps.desc': 'A notice each time it turns on or off.',
    'islands.power.title': 'Power and battery', 'islands.power.desc': 'When the charger is plugged in or out, and when the battery is full or low.',
    'islands.drives.title': 'Drives', 'islands.drives.desc': 'When a drive arrives or leaves. A drive that is plugged in can be ejected from the menu bar panel.',
    'islands.downloads.title': 'Finished downloads', 'islands.downloads.desc': 'When a file in the Downloads folder has stopped growing. Only names and sizes are looked at.',
    'islands.camera.title': 'Camera', 'islands.camera.desc': 'Stays on the notch while an app is using the camera. It says that it is in use, not which app.',
    'islands.microphone.title': 'Microphone', 'islands.microphone.desc': 'Stays on the notch while an app is using the microphone. It says that it is in use, not which app.',
    'islands.sample': 'Show a sample',
    'sec.islands.battery': 'Battery', 'islands.full.title': 'Full at', 'islands.full.desc': 'Say the battery is full when it reaches this charge while plugged in.',
    'islands.low.title': 'Low at', 'islands.low.desc': 'Say the battery is low when it falls to this charge on battery.',
    'islands.nobattery': 'This Mac has no battery, so the notices about it never appear.',
    'sec.islands.time': 'Time', 'islands.seconds.title': 'How long a notice stays', 'islands.seconds.desc': 'The camera and microphone notices stay as long as they are in use.',
  },
  vi: {
    'nav.islands': 'Đảo trạng thái', 'page.islands': 'Đảo trạng thái',
    'page.islands.sub': 'Thông báo nhỏ hiện ra từ notch đang đóng trong chốc lát: Caps Lock, sạc, pin đầy hoặc yếu, ổ đĩa, tệp tải xong, camera hay micro đang được dùng.',
    'islands.enable.title': 'Hiện đảo trạng thái', 'islands.enable.desc': 'Không có gì ở đây đọc nội dung màn hình hay những gì bạn gõ. Mỗi thông báo đều tắt riêng được ở dưới.',
    'sec.islands.which': 'Những thông báo nào',
    'islands.caps.title': 'Caps Lock', 'islands.caps.desc': 'Một thông báo mỗi lần bật hoặc tắt.',
    'islands.power.title': 'Sạc và pin', 'islands.power.desc': 'Khi cắm hoặc rút sạc, và khi pin đầy hoặc yếu.',
    'islands.drives.title': 'Ổ đĩa', 'islands.drives.desc': 'Khi một ổ đĩa được cắm vào hoặc tháo ra. Ổ đang cắm có thể tháo từ bảng ở thanh menu.',
    'islands.downloads.title': 'Tải xong', 'islands.downloads.desc': 'Khi một tệp trong thư mục Downloads đã ngừng lớn thêm. Chỉ xem tên và dung lượng.',
    'islands.camera.title': 'Camera', 'islands.camera.desc': 'Nằm trên notch khi có ứng dụng đang dùng camera. Chỉ báo là có, không nói ứng dụng nào.',
    'islands.microphone.title': 'Micro', 'islands.microphone.desc': 'Nằm trên notch khi có ứng dụng đang dùng micro. Chỉ báo là có, không nói ứng dụng nào.',
    'islands.sample': 'Xem thử',
    'sec.islands.battery': 'Pin', 'islands.full.title': 'Đầy ở mức', 'islands.full.desc': 'Báo pin đầy khi đạt mức sạc này lúc đang cắm sạc.',
    'islands.low.title': 'Yếu ở mức', 'islands.low.desc': 'Báo pin yếu khi tụt xuống mức này lúc dùng pin.',
    'islands.nobattery': 'Máy này không có pin, nên các thông báo về pin không bao giờ hiện.',
    'sec.islands.time': 'Thời gian', 'islands.seconds.title': 'Thông báo hiện bao lâu', 'islands.seconds.desc': 'Thông báo camera và micro ở lại chừng nào còn được dùng.',
  },
  build() {
    const on = () => S.islands.enabled;
    const sample = (kind) => button(t('islands.sample'), () => send({ t: 'action', name: 'islands_sample:' + kind }), { small: true });
    const notice = (key, kind) => row(t('islands.' + key + '.title'), t('islands.' + key + '.desc'),
      [sample(kind), switchEl('islands.' + (key === 'caps' ? 'caps_lock' : key), t('islands.' + key + '.title'))], { when: on });

    const full = sliderRow({ path: 'islands.full_at', title: t('islands.full.title'), desc: t('islands.full.desc'), min: 80, max: 100, step: 1, format: (v) => v + '%' });
    const low = sliderRow({ path: 'islands.low_at', title: t('islands.low.title'), desc: t('islands.low.desc'), min: 5, max: 30, step: 1, format: (v) => v + '%' });
    const battery = card(full, low);
    const none = card(h('div', { class: 'islands-note', text: t('islands.nobattery') }));
    const seconds = sliderRow({ path: 'islands.seconds', title: t('islands.seconds.title'), desc: t('islands.seconds.desc'), min: 2, max: 10, step: 0.5, format: (v) => v.toFixed(1) + ' s' });
    reg(() => {
      battery.style.display = S.islands.has_battery ? '' : 'none';
      none.style.display = S.islands.has_battery ? 'none' : '';
      for (const el of [full, low, seconds]) el.classList.toggle('disabled', !on());
    });
    return [
      card(toggleRow('islands.enabled', t('islands.enable.title'), t('islands.enable.desc'))),
      sectionTitle(t('sec.islands.which')),
      card(notice('caps', 'caps'), notice('power', 'plug'), notice('drives', 'drive'), notice('downloads', 'download'), notice('camera', 'camera'), notice('microphone', 'mic')),
      sectionTitle(t('sec.islands.battery')),
      battery,
      none,
      sectionTitle(t('sec.islands.time')),
      card(seconds),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('islands');
    const page = document.querySelector('#scroller');
    sent.length = 0;
    const master = page.querySelector('.switch');
    const was = S.islands.enabled;
    master.click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'islands.enabled' && m.value === !was), 'islands: the switch sends its setting: ' + JSON.stringify(sent));
    master.click();

    // Each notice has a switch of its own and a sample.
    expect(page.querySelectorAll('.switch').length === 7, 'islands: one switch for the feature and one for each of six notices');
    const samples = [...page.querySelectorAll('.btn')].filter((b) => b.textContent === t('islands.sample'));
    expect(samples.length === 6, 'islands: a sample for each notice');
    sent.length = 0;
    const mic = [...page.querySelectorAll('.row')].find((r) => r.textContent.includes(t('islands.microphone.title')) && !r.textContent.includes(t('islands.camera.title')));
    mic.querySelector('.switch').click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'islands.microphone'), 'islands: the microphone has a switch of its own: ' + JSON.stringify(sent));
    mic.querySelector('.switch').click();
    sent.length = 0;
    samples[5].click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'islands_sample:mic'), 'islands: the microphone sample sends its action: ' + JSON.stringify(sent));
    sent.length = 0;
    samples[1].click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'islands_sample:plug'), 'islands: the power sample sends its action: ' + JSON.stringify(sent));

    // The battery levels are there only on a Mac that has a battery.
    const sliders = () => [...page.querySelectorAll('input[type=range]')].filter((i) => i.offsetParent !== null || true);
    expect(sliders().length === 3, 'islands: two battery levels and the time');
    S.islands.has_battery = false;
    runSyncs();
    expect([...page.querySelectorAll('.card')].some((c) => c.textContent.includes(t('islands.nobattery')) && c.style.display !== 'none'), 'islands: a Mac without a battery is told so');
    S.islands.has_battery = true;
    runSyncs();
  },
});
