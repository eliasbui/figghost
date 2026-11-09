import { Fragment } from 'react';
import { Button, IconButton, Segmented, SegRow, Switch, SwitchButton, ToggleRow } from '@/components/macos/Controls';
import { Card, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { AppHead, PageIcon } from '@/components/macos/Page';
import { resolveLang } from '@/i18n';
import { masterPath, NAV, TAB_PAGES } from '@/pages/meta';
import { change, openPage, send, useApp } from '@/stores/app';
import { useSetting, useT } from '@/stores/hooks';

/** One feature in the list: the switch turns it on and off, the name opens its page. */
function FeatureLine({ id }: { id: string }) {
  const t = useT();
  const master = useApp((s) => masterPath(id, s.state));
  const name = t(`nav.${id}`);
  return (
    <div
      className="row feat-row"
      data-page={id}
      onClick={(e) => {
        if (!(e.target as Element).closest('button')) openPage(id);
      }}
    >
      <div className="row-lead">
        <PageIcon id={id} />
      </div>
      <div className="row-text">
        <button type="button" className="feat-name" onClick={() => openPage(id)}>
          {name}
        </button>
      </div>
      <div className="row-control">
        {master ? <Switch path={master} label={name} /> : null}
        <span className="chev" aria-hidden="true" />
      </div>
    </div>
  );
}

/** The features, grouped as the sidebar is and then the tabs of the notch. */
function FeatureList() {
  const t = useT();
  const groups = [...NAV.slice(1), { sec: 'tabs', pages: TAB_PAGES }];
  return (
    <>
      {groups.map(({ sec, pages }) => (
        <Fragment key={sec}>
          <SectionTitle minor>{t(`nav.sec.${sec}`)}</SectionTitle>
          <Card>
            {pages.map((id) => (
              <FeatureLine key={id} id={id} />
            ))}
          </Card>
        </Fragment>
      ))}
    </>
  );
}

export default function General() {
  const t = useT();
  const env = useApp((s) => s.state.env);
  const version = useApp((s) => s.state.app.version);
  const language = useSetting<string>('general.language');
  const notchOn = useSetting<boolean>('notch.enabled');

  // Launch at login depends on how the app was installed, so its row has more states than a plain switch.
  const loginOn = env.login === 'enabled' || env.login === 'needs_approval';
  const needsApproval = env.login === 'needs_approval';
  const unavailable = env.login === 'unavailable';

  return (
    <>
      <AppHead line={t('chip.version', { v: version })}>
        {/* Checking for a new version shows its progress where it lives, on About. */}
        <IconButton
          icon="refresh"
          variant="glass"
          label={t('about.update.check')}
          onClick={() => {
            send({ t: 'action', name: 'check_updates' });
            openPage('about');
          }}
        />
      </AppHead>

      <SectionTitle>{t('sec.permissions')}</SectionTitle>
      <Card>
        <Row title={t('access.title')} desc={t('access.desc')}>
          <Pill good={env.accessibility}>{t(env.accessibility ? 'access.granted' : 'access.missing')}</Pill>
          <IconButton icon="open" variant="glass" label={t('access.open')} onClick={() => send({ t: 'action', name: 'open_accessibility' })} />
        </Row>
        <Row title={t('perm.all_title')} desc={t('perm.all_desc')}>
          <IconButton icon="checkmark.shield" variant="glass" label={t('perm.all_btn')} onClick={() => send({ t: 'action', name: 'check_permissions' })} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.startup')}</SectionTitle>
      <Card>
        <Row title={t('login.title')} desc={t('login.desc')} below={needsApproval ? t('login.approval') : unavailable ? t('login.unavailable') : null} when={!unavailable}>
          {needsApproval ? (
            <IconButton icon="open" variant="glass" label={t('login.open')} onClick={() => send({ t: 'action', name: 'open_login_settings' })} />
          ) : null}
          <SwitchButton
            checked={loginOn}
            disabled={unavailable}
            label={t('login.title')}
            // The login item is not a setting of the file: the app does it, and says in the state how it went.
            onToggle={() => send({ t: 'set', path: 'launch_at_login', value: !loginOn })}
          />
        </Row>
        <ToggleRow path="general.show_on_launch" title={t('showlaunch.title')} desc={t('showlaunch.desc')} />
        <ToggleRow path="general.launch_effect" title={t('hello.title')} desc={t('hello.desc')} when={notchOn} />
        <ToggleRow path="general.menu_icon" title={t('menuicon.title')} desc={t('menuicon.desc')} />
      </Card>

      <SectionTitle>{t('sec.appearance')}</SectionTitle>
      <Card>
        <SegRow path="general.theme" title={t('theme.title')} desc={t('theme.desc')} values={['auto', 'light', 'dark']} />
        <Row title={t('lang.title')} desc={t('lang.desc')}>
          <Segmented
            label={t('lang.title')}
            value={language}
            options={[
              { value: 'auto', label: t('lang.auto') },
              { value: 'en', label: t('lang.en') },
              { value: 'vi', label: t('lang.vi') },
            ]}
            onPick={(v) => change('general.language', v)}
          />
        </Row>
        {resolveLang(language) !== env.language ? (
          <Row title={t('lang.restart')}>
            <Button variant="prominent" icon="refresh" onClick={() => send({ t: 'action', name: 'restart' })}>
              {t('lang.restart.btn')}
            </Button>
          </Row>
        ) : null}
      </Card>

      <SectionTitle>{t('feat.title')}</SectionTitle>
      <FeatureList />

      <SectionTitle>{t('sec.data')}</SectionTitle>
      <Card>
        <Row title={t('data.title')} desc={t('data.desc')}>
          <IconButton icon="folder" variant="glass" label={t('data.open')} onClick={() => send({ t: 'action', name: 'open_config' })} />
        </Row>
      </Card>
    </>
  );
}
