// Pull requests: a tab of the notch with the review requests that wait for you on GitHub and GitLab.
(window.__plugins = window.__plugins || []).push({
  id: 'prs',
  icon: 'arrow.triangle.pull',
  en: {
    'nav.prs': 'Pull requests', 'page.prs': 'Pull requests',
    'page.prs.sub': 'See the review requests that wait for you on GitHub and GitLab.',
    'prs.enable.title': 'Show the Pull requests tab', 'prs.enable.desc': 'The requests newest first, with a dot on the tab while there are some. Press one to open it in the browser. Figghost never sees your password or token: it asks the gh and glab tools you signed in with, and keeps nothing on disk.',
    'sec.prs.where': 'Where to look',
    'prs.github.title': 'GitHub', 'prs.github.desc': 'Pull requests where your review is requested, through the gh tool.',
    'prs.gitlab.title': 'GitLab', 'prs.gitlab.desc': 'Merge requests where your review is requested, through the glab tool.',
    'prs.tool.ready': 'Signed in', 'prs.tool.signed_out': 'Not signed in', 'prs.tool.missing': 'Not installed', 'prs.tool.slow': 'No answer', 'prs.tool.unknown': 'Not checked',
    'prs.check': 'Check', 'prs.checking': 'Checking...',
    'sec.prs.list': 'The list',
    'prs.drafts.title': 'Include drafts', 'prs.drafts.desc': 'A draft is not ready for review yet, so it is left out unless you ask.',
    'prs.minutes.title': 'Look again every', 'prs.minutes.desc': 'While this is on, the tools are asked this often, so that the dot on the tab is right.', 'prs.minutes.n': '{n} min',
    'prs.count': '{n} waiting now.',
  },
  vi: {
    'nav.prs': 'Yêu cầu duyệt', 'page.prs': 'Yêu cầu duyệt',
    'page.prs.sub': 'Xem các yêu cầu duyệt đang chờ bạn trên GitHub và GitLab.',
    'prs.enable.title': 'Hiện tab Yêu cầu duyệt', 'prs.enable.desc': 'Yêu cầu mới nhất ở trên, và có một chấm trên tab khi có yêu cầu. Bấm vào một dòng để mở trong trình duyệt. Figghost không bao giờ thấy mật khẩu hay token của bạn: nó hỏi các công cụ gh và glab bạn đã đăng nhập, và không lưu gì trên đĩa.',
    'sec.prs.where': 'Xem ở đâu',
    'prs.github.title': 'GitHub', 'prs.github.desc': 'Pull request đang chờ bạn duyệt, qua công cụ gh.',
    'prs.gitlab.title': 'GitLab', 'prs.gitlab.desc': 'Merge request đang chờ bạn duyệt, qua công cụ glab.',
    'prs.tool.ready': 'Đã đăng nhập', 'prs.tool.signed_out': 'Chưa đăng nhập', 'prs.tool.missing': 'Chưa cài', 'prs.tool.slow': 'Không trả lời', 'prs.tool.unknown': 'Chưa kiểm tra',
    'prs.check': 'Kiểm tra', 'prs.checking': 'Đang kiểm tra...',
    'sec.prs.list': 'Danh sách',
    'prs.drafts.title': 'Gồm cả bản nháp', 'prs.drafts.desc': 'Bản nháp chưa sẵn sàng để duyệt, nên mặc định được bỏ qua.',
    'prs.minutes.title': 'Xem lại mỗi', 'prs.minutes.desc': 'Khi đang bật, các công cụ được hỏi chừng này thời gian một lần, để chấm trên tab luôn đúng.', 'prs.minutes.n': '{n} phút',
    'prs.count': 'Hiện có {n} yêu cầu đang chờ.',
  },
  build() {
    const on = () => S.prs.enabled;
    const when = { when: on };
    const minutes = () => [5, 10, 30, 60].map((n) => ({ value: n, label: t('prs.minutes.n', { n }) }));
    const gh = h('span', { class: 'prs-tool' });
    const glab = h('span', { class: 'prs-tool' });
    const check = button(t('prs.check'), () => send({ t: 'action', name: 'prs_check' }), { small: true });
    const count = h('div', { class: 'row-desc' });
    reg(() => {
      const p = S.prs;
      gh.textContent = t('prs.tool.' + p.github_tool);
      glab.textContent = t('prs.tool.' + p.gitlab_tool);
      gh.dataset.state = p.github_tool;
      glab.dataset.state = p.gitlab_tool;
      check.textContent = p.checking ? t('prs.checking') : t('prs.check');
      check.disabled = p.checking;
      count.textContent = t('prs.count', { n: p.count });
    });
    return [
      card(toggleRow('prs.enabled', t('prs.enable.title'), t('prs.enable.desc'))),
      sectionTitle(t('sec.prs.where')),
      card(
        row(t('prs.github.title'), t('prs.github.desc'), [gh, switchEl('prs.github', t('prs.github.title'))], when),
        row(t('prs.gitlab.title'), t('prs.gitlab.desc'), [glab, switchEl('prs.gitlab', t('prs.gitlab.title'))], when),
        row(t('prs.check'), null, check, when)),
      sectionTitle(t('sec.prs.list')),
      card(
        toggleRow('prs.drafts', t('prs.drafts.title'), t('prs.drafts.desc'), when),
        selectRow({ path: 'prs.minutes', title: t('prs.minutes.title'), desc: t('prs.minutes.desc'), options: minutes, when: on }),
        row(null, null, count, { block: true, when: on })),
    ];
  },
  selftest({ expect, go, sent, S }) {
    go('prs');
    const page = document.querySelector('#scroller');
    const switches = [...page.querySelectorAll('.switch')];
    expect(switches.length === 4, 'prs: one switch for the tab, one for each service and one for drafts');
    sent.length = 0;
    const was = S.prs.enabled;
    switches[0].click();
    expect(sent.some((m) => m.t === 'set' && m.path === 'prs.enabled' && m.value === !was), 'prs: the switch sends its setting: ' + JSON.stringify(sent));
    switches[0].click();
    sent.length = 0;
    switches[1].click();
    switches[2].click();
    expect(sent.some((m) => m.path === 'prs.github') && sent.some((m) => m.path === 'prs.gitlab'), 'prs: each service has its own switch: ' + JSON.stringify(sent));
    switches[1].click();
    switches[2].click();

    // The state of each tool is said in words.
    expect(page.textContent.includes(t('prs.tool.ready')) && page.textContent.includes(t('prs.tool.signed_out')), 'prs: each tool says whether it is signed in');
    S.prs.gitlab_tool = 'missing'; runSyncs();
    expect(page.textContent.includes(t('prs.tool.missing')), 'prs: a tool that is not installed says so');
    S.prs.gitlab_tool = 'signed_out'; runSyncs();

    const pick = page.querySelector('select.pick');
    expect(pick.options.length === 4, 'prs: four intervals to choose from');
    pick.value = '30';
    pick.dispatchEvent(new Event('change'));
    expect(S.prs.minutes === 30, 'prs: the interval is a number of minutes');
    pick.value = '10';
    pick.dispatchEvent(new Event('change'));

    sent.length = 0;
    const check = [...page.querySelectorAll('.btn')].find((b) => b.textContent === t('prs.check'));
    check.click();
    expect(sent.some((m) => m.t === 'action' && m.name === 'prs_check'), 'prs: the check button sends its action: ' + JSON.stringify(sent));
    S.prs.checking = true; runSyncs();
    expect(check.disabled && check.textContent === t('prs.checking'), 'prs: the button waits while a check is running');
    S.prs.checking = false; runSyncs();
  },
});
