import { useState } from 'react';
import { Button, IconButton, Switch, ToggleRow } from '@/components/macos/Controls';
import { Advanced } from '@/components/macos/Dialogs';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { AppHead } from '@/components/macos/Page';
import license from '@/assets/LICENSE.txt?raw';
import { bytes } from '@/lib/text';
import { send, useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import type { TFunction } from '@/stores/hooks';

// The versions the history lists, newest first; each has a title and a body in the dictionary.
const VERSIONS = ['2_0_2', '2_0', '1_x', '1_0', '0_9', '0_8', '0_7', '0_6', '0_5', '0_4', '0_2'];

const USAGE_KEYS = ['cpu', 'gpu', 'memory', 'net', 'disk', 'fps'] as const;

/** The update row: a button that looks for a new version, and what is happening to the one it found. */
function UpdateRow({ version }: { version: string }) {
  const t = useT();
  const release = useApp((s) => s.update);
  const { state, version: v = '', detail = '' } = release;
  const desc =
    state === 'checking'
      ? t('about.update.checking')
      : state === 'current'
        ? t('about.update.current', { v: version })
        : state === 'available'
          ? t('about.update.available', { v })
          : state === 'installing'
            ? t(release.stage === 'downloading' ? 'about.update.downloading' : 'about.update.installing', { v, p: release.percent || 0 })
            : state === 'ready'
              ? t('about.update.ready', { v })
              : state === 'blocked'
                ? t('about.update.blocked', { v, detail })
                : state === 'failed'
                  ? t('about.update.failed', { detail })
                  : t('about.update.desc');
  const ask = (name: string) => () => send({ t: 'action', name });
  return (
    <Row title={t('about.update')} desc={t('about.update.desc')} below={desc === t('about.update.desc') ? null : desc}>
      {state === 'available' ? (
        <Button small primary onClick={ask('install_update')}>
          {t('about.update.install')}
        </Button>
      ) : null}
      {state === 'ready' ? (
        <Button small primary onClick={ask('restart')}>
          {t('about.update.restart')}
        </Button>
      ) : null}
      {state === 'blocked' ? (
        <Button small primary onClick={ask('open_release')}>
          {t('about.update.open')}
        </Button>
      ) : null}
      <IconButton variant="glass" icon="refresh" loading={state === 'checking' || state === 'installing'} label={t('about.update.check')} onClick={ask('check_updates')} />
    </Row>
  );
}

const percent = (v: number) => `${v < 10 ? v.toFixed(1) : Math.round(v)}%`;

/** What Figghost itself uses, as numbers that follow the state. */
function UsageCard() {
  const t = useT();
  const usage = useApp((s) => s.state.app.usage);
  const shown: Record<(typeof USAGE_KEYS)[number], (u: Record<string, number>) => string> = {
    cpu: (u) => percent(u.cpu),
    gpu: (u) => percent(u.gpu),
    memory: (u) => bytes(u.memory),
    net: (u) => `${bytes(u.net)}/s`,
    disk: (u) => t('about.usage.disk.n', { r: bytes(u.disk_read), w: bytes(u.disk_write) }),
    fps: (u) => t('about.usage.fps.n', { n: Math.round(u.fps) }),
  };
  return (
    <>
      <SectionTitle info={t('about.usage.desc')}>{t('about.usage.title')}</SectionTitle>
      <Card>
        {USAGE_KEYS.map((key) => (
          <Row key={key} title={t(`about.usage.${key}`)}>
            <span className="row-value">{usage && usage[key === 'disk' ? 'disk_read' : key] != null ? shown[key](usage) : t('about.usage.wait')}</span>
          </Row>
        ))}
      </Card>
    </>
  );
}

function Licence({ t }: { t: TFunction }) {
  const [open, setOpen] = useState(false);
  return (
    <Card>
      <Row title={t('about.license.title')} desc={t('about.license.body')}>
        <Button small aria-expanded={open} onClick={() => setOpen((now) => !now)}>
          {t(open ? 'about.license.hide' : 'about.license.show')}
        </Button>
      </Row>
      <pre className="license" hidden={!open}>
        {license.trimEnd()}
      </pre>
      <Row title={t('about.others.title')} desc={t('about.others.body')} />
    </Card>
  );
}

export default function About() {
  const t = useT();
  const app = useApp((s) => s.state.app);
  return (
    <>
      <AppHead line={t('chip.version', { v: app.version })} />
      <Card>
        <UpdateRow version={app.version} />
        <ToggleRow path="update.auto" title={t('update.auto.title')} desc={t('update.auto.desc')} />
        <Row title={t('update.news.title')} desc={t('update.news.desc')}>
          <IconButton variant="glass" icon="sparkles" label={t('update.news.show')} onClick={() => send({ t: 'action', name: 'show_whats_new' })} />
          <Switch path="update.whats_new" label={t('update.news.title')} />
        </Row>
        <Row title={t('about.privacy')} desc={t('about.privacy.val')} />
        <Row title={t('data.title')} desc={app.config_dir}>
          <IconButton variant="glass" icon="folder" label={t('data.open')} onClick={() => send({ t: 'action', name: 'open_config' })} />
        </Row>
      </Card>

      <UsageCard />

      <SectionTitle>{t('about.sec.license')}</SectionTitle>
      <Licence t={t} />
      <Card>
        <Row title={t('about.quit')} desc={t('about.quit.desc')}>
          <IconButton variant="danger" icon="power" label={t('about.quit')} onClick={() => send({ t: 'action', name: 'quit' })} />
        </Row>
      </Card>

      <Advanced>
        <SectionTitle>{t('about.sec.what')}</SectionTitle>
        <Card>
          <Row title={t('about.what.title')} desc={t('about.what.body')} />
          <Row title={t('about.native.title')} desc={t('about.native.body')} />
        </Card>
        <SectionTitle>{t('about.sec.history')}</SectionTitle>
        <Card>
          <Row title={t('about.why.title')} desc={t('about.why.body')} />
        </Card>
        <Card>
          <Row title={t('about.versions.title')} />
          {VERSIONS.map((v) => (
            <Row key={v} title={t(`about.v.${v}`)} desc={t(`about.v.${v}.body`)} />
          ))}
        </Card>
      </Advanced>
    </>
  );
}
