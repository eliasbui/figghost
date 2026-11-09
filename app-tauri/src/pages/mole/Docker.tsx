import { Button, IconButton } from '@/components/macos/Controls';
import { confirmDialog } from '@/components/macos/Dialogs';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { size } from './format';
import { PickList } from './PickList';
import type { Item, MoleState } from './types';

const act = (name: string) => send({ t: 'action', name });

const PARTS = ['Images', 'Containers', 'Local Volumes', 'Build Cache', 'logs'];
const clsOf = (k: string) => `d-${k.split(' ').pop()!.toLowerCase()}`;

/** What Docker takes, with the clean-ups that work on all of it at once. */
export function DockerCard({ mole }: { mole: MoleState }) {
  const t = useT();
  const d = mole.docker;
  const ok = !!(d && d.ok);
  const busy = (k: string) => mole.busy.includes(k);
  const working = busy('docker_act') || busy('docker');
  const ask = (label: string, onYes: () => void) => confirmDialog({ title: t(`mole.docker.${label}`), body: t(`mole.docker.${label}.body`), confirm: t(`mole.docker.${label}`) }, onYes);
  const note = !d || busy('docker') ? t('mole.scanning') : d.missing ? t('mole.docker.missing') : d.down ? t('mole.docker.down') : d.disk ? t('mole.docker.disk', { size: size(d.disk) }) : '';

  const parts = ok
    ? PARTS.map((k): [string, { size: number; reclaimable: number }] => [k, k === 'logs' ? { size: d.logs, reclaimable: d.logs } : (d.totals ?? []).find((x) => x.type === k) ?? { size: 0, reclaimable: 0 }])
    : [];
  const whole = parts.reduce((a, [, r]) => a + r.size, 0);

  return (
    <>
      <SectionTitle>{t('mole.sec.docker')}</SectionTitle>
      <Card>
        <Row title={t('mole.docker.title')} desc={t('mole.docker.desc')} />
        <div className="mole-free">{note}</div>
        <div className="mole-free" hidden={!ok}>
          <div className="mole-tile-head">
            <span className="mole-tile-sub">{t('mole.docker.reclaim', { size: size(parts.reduce((a, [, r]) => a + r.reclaimable, 0)) })}</span>
            <b>{size(whole)}</b>
          </div>
          <div className="mole-stack big">
            {parts.map(([k, r]) => (
              <span key={k} className={clsOf(k)} style={{ width: `${whole ? (r.size / whole) * 100 : 0}%` }} />
            ))}
          </div>
          <div className="mole-legend">
            {parts.map(([k, r]) => (
              <span key={k} className={clsOf(k)}>
                <i />
                {`${t(`mole.docker.t.${k}`)} ${size(r.size)}`}
              </span>
            ))}
          </div>
        </div>
        <div className="mole-toolbar">
          <span className="mole-all" />
          <Button small hidden={!ok} disabled={working || !ok || !d.containers.length} onClick={() => ask('clear_logs', () => act(`mole_docker:logs\n${d!.containers.map((c) => c.path).join('\n')}`))}>
            {t('mole.docker.clear_logs')}
          </Button>
          <Button small danger icon="trash" hidden={!ok} disabled={working} onClick={() => ask('unused', () => act('mole_docker_prune:unused'))}>
            {t('mole.docker.unused')}
          </Button>
          <Button small danger icon="trash" hidden={!ok} disabled={working} onClick={() => ask('cache', () => act('mole_docker_prune:cache'))}>
            {t('mole.docker.cache')}
          </Button>
          <IconButton size="sm" variant="glass" icon="refresh" label={t('mole.refresh')} disabled={busy('docker')} onClick={() => act('mole_scan:docker')} />
        </div>
      </Card>
    </>
  );
}

/** The containers, images, volumes and networks of Docker, each a list to choose from. */
export function DockerLists({ mole }: { mole: MoleState }) {
  const t = useT();
  const d = mole.docker;
  const ok = !!(d && d.ok);
  const read = (k: 'containers' | 'images' | 'volumes' | 'networks'): Item[] | null => (ok ? d![k] : null);
  const used = (i: Item) => (i.used ? t('mole.docker.used', { n: i.used }) : t('mole.docker.unused.tag'));
  const common = { home: mole.home, busy: mole.busy, scan: 'docker' };
  return (
    <div className="mole-docker" hidden={!ok}>
      <PickList
        {...common}
        kind="docker_containers"
        items={read('containers')}
        filter
        lead={(i) => <span className={`mole-dot ${i.state ?? ''}`} title={i.state} />}
        detail={(i) => [i.note, i.status, i.logs ? t('mole.docker.logs', { size: size(i.logs) }) : ''].filter(Boolean).join(' / ')}
        actions={[
          { verb: 'start', label: 'start' },
          { verb: 'stop', label: 'stop' },
          { verb: 'restart', label: 'restart' },
          { verb: 'logs', label: 'clean', danger: true },
          { verb: 'rm', label: 'remove', danger: true },
        ]}
      />
      <PickList {...common} kind="docker_images" items={read('images')} filter detail={(i) => [i.note, used(i)].join(' / ')} actions={[{ verb: 'rmi', label: 'remove', danger: true }]} />
      <PickList
        {...common}
        kind="docker_volumes"
        items={read('volumes')}
        filter
        detail={(i) => [i.anonymous ? t('mole.docker.anonymous') : '', used(i)].filter(Boolean).join(' / ')}
        actions={[{ verb: 'volume_rm', label: 'remove', danger: true }]}
      />
      <PickList {...common} kind="docker_networks" items={read('networks')} detail={(i) => i.note} actions={[{ verb: 'network_rm', label: 'remove', danger: true }]} />
    </div>
  );
}
