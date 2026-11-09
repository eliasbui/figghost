import { useEffect, useRef, useState } from 'react';
import { ActionButton, Button, SelectRow, ToggleRow } from '@/components/macos/Controls';
import { Card, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { change, send, showToast } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './download.css';

interface Job {
  id: string;
  url: string;
  title?: string;
  status: 'queued' | 'running' | 'done' | 'failed' | 'cancelled';
  percent: number | null;
  speed?: number;
  eta: number | null;
  error?: string;
  needs_ffmpeg?: boolean;
}
interface NetLink {
  name: string;
  label: string;
  state: string;
  primary?: boolean;
  connections?: number;
  speed: number;
}
interface DownloadState {
  enabled: boolean;
  ytdlp: string;
  ytdlp_version?: string;
  ffmpeg: string;
  folder: string;
  folder_path: string;
  picked: { seq: number; path?: string };
  net_merge: boolean;
  net_paused?: boolean;
  net_links?: NetLink[];
  jobs: Job[];
  lanes_installed: boolean;
  lanes_running: boolean;
}

const bytes = (n: number) => {
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.min(units.length - 1, Math.floor(Math.log(Math.max(n, 1)) / Math.log(1024)));
  return `${(n / 1024 ** i).toFixed(i > 1 ? 1 : 0)} ${units[i]}`;
};
const clock = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
const hostOf = (url: string) => url.replace(/^[a-z]+:\/\//i, '').split(/[/?#]/)[0];

function JobRow({ job }: { job: Job }) {
  const t = useT();
  const pct = job.percent ?? 0;
  let text = t(`download.st.${job.status}`);
  if (job.status === 'running' && job.percent != null) {
    text = [`${Math.round(pct)}%`, job.speed ? t('download.speed', { speed: bytes(job.speed) }) : '', job.eta != null ? t('download.eta', { time: clock(job.eta) }) : ''].filter(Boolean).join(' / ');
  } else if (job.status === 'failed') {
    text = job.needs_ffmpeg ? t('download.err.ffmpeg') : `${t('download.st.failed')}: ${job.error ?? ''}`;
  }
  return (
    <div className="download-item" data-status={job.status} title={job.url}>
      <span className="download-text">
        <span className="download-name">{job.title || hostOf(job.url)}</span>
        <span className="download-note">{text}</span>
        <span className="download-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)} hidden={job.status !== 'running'}>
          <span style={{ width: `${pct}%` }} />
        </span>
      </span>
      <span className="download-tools">
        <ActionButton name={`download_cancel:${job.id}`} icon="xmark" hidden={!(job.status === 'queued' || job.status === 'running')}>
          {t('download.cancel')}
        </ActionButton>
        <ActionButton name={`download_reveal:${job.id}`} icon="folder" hidden={job.status !== 'done'}>
          {t('download.reveal')}
        </ActionButton>
      </span>
    </div>
  );
}

/** Video download: paste a link and yt-dlp, which you installed yourself, saves the video to your Downloads folder. At the
 *  foot, Lanes, the download manager by the same author, and a switch for its downloads on the notch. */
export default function Download() {
  const t = useT();
  const d = useSection<DownloadState>('download');
  const on = d.enabled;
  const ready = on && !!d.ytdlp;
  const [link, setLink] = useState('');
  const submit = () => {
    const address = link.trim();
    if (!/^https?:\/\/[^\s\u0000-\u001f]+$/i.test(address)) {
      showToast(t('fault.download_bad_url'));
      return;
    }
    send({ t: 'action', name: `download_add:${address}` });
    setLink('');
  };

  // The panel that chooses a folder is the app's, so the choice comes back through the state and is stored from here.
  const seen = useRef(d.picked.seq);
  useEffect(() => {
    if (d.picked.seq === seen.current) return;
    seen.current = d.picked.seq;
    if (d.picked.path) change('download.folder', d.picked.path);
  }, [d.picked.seq, d.picked.path]);

  const links = d.net_links ?? [];
  const netNote = d.net_paused ? t('download.net.paused') : !links.length ? t('download.net.none') : links.length === 1 ? t('download.net.one') : '';
  const finished = d.jobs.some((j) => j.status === 'done' || j.status === 'failed' || j.status === 'cancelled');

  return (
    <>
      <SectionTitle>{t('sec.download.tools')}</SectionTitle>
      <Card>
        <Row title={t('download.ytdlp.title')} desc={d.ytdlp ? (d.ytdlp_version ? t('download.ytdlp.found', { version: d.ytdlp_version }) : t('download.ytdlp.found.plain')) : t('download.ytdlp.missing')} state={d.ytdlp ? 'ready' : 'missing'}>
          {d.ytdlp ? (
            <span className="row-value" title={d.ytdlp}>
              {d.ytdlp}
            </span>
          ) : null}
          <Pill good={!!d.ytdlp}>{t(d.ytdlp ? 'ui.found' : 'ui.missing')}</Pill>
        </Row>
        <Row title={t('download.ffmpeg.title')} desc={d.ffmpeg ? t('download.ffmpeg.found') : t('download.ffmpeg.missing')} state={d.ffmpeg ? 'ready' : 'missing'}>
          {d.ffmpeg ? (
            <span className="row-value" title={d.ffmpeg}>
              {d.ffmpeg}
            </span>
          ) : null}
          <Pill good={!!d.ffmpeg}>{t(d.ffmpeg ? 'ui.found' : 'ui.missing')}</Pill>
        </Row>
      </Card>

      <SectionTitle>{t('sec.download.get')}</SectionTitle>
      <Card>
        <div className="row block">
          <div className="download-link">
            <input
              type="text"
              className="text-field"
              aria-label={t('download.link.label')}
              placeholder={t('download.link.ph')}
              maxLength={2048}
              spellCheck={false}
              autoComplete="off"
              autoCapitalize="off"
              disabled={!ready}
              value={link}
              onChange={(e) => setLink(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.nativeEvent.isComposing) submit();
              }}
            />
            <Button primary small icon="arrow.down.circle.fill" disabled={!ready} onClick={submit}>
              {t('download.go')}
            </Button>
          </div>
        </div>
        <SelectRow
          path="download.quality"
          title={t('download.quality.title')}
          desc={t('download.quality.desc')}
          when={on}
          options={['best', '1080', '720', 'audio'].map((q) => ({ value: q, label: t(`download.quality.${q}`) }))}
        />
        <ToggleRow path="download.playlist" title={t('download.playlist.title')} desc={t('download.playlist.desc')} when={on} />
        <Row title={t('download.folder.title')} when={on}>
          <div className="download-folder">
            <span className="download-path">{d.folder_path}</span>
            <Button small hidden={!d.folder} onClick={() => change('download.folder', '')}>
              {t('download.folder.reset')}
            </Button>
            <ActionButton name="download_pick">{t('download.folder.choose')}</ActionButton>
          </div>
        </Row>
      </Card>

      <SectionTitle>{t('sec.download.net')}</SectionTitle>
      <Card>
        <ToggleRow path="download.net_merge" title={t('download.net.title')} desc={t('download.net.desc')} when={on} />
        <div className="download-links" hidden={!(on && d.net_merge)}>
          {links.map((l) => (
            <div className="download-net" key={l.name} data-state={l.state}>
              <span className="download-net-name">
                {l.label === l.name ? l.label : `${l.label} (${l.name})`}
                {l.primary ? <span className="download-net-tag">{t('download.net.primary')}</span> : null}
              </span>
              <span className="download-net-state">{l.connections ? t('download.net.busy', { n: l.connections, speed: bytes(l.speed) }) : t(`download.net.st.${l.state}`)}</span>
            </div>
          ))}
          {netNote ? (
            <div className="download-net-note" role="status">
              {netNote}
            </div>
          ) : null}
        </div>
      </Card>

      <SectionTitle>{t('sec.download.list')}</SectionTitle>
      <Card>
        <div className="download-list">
          {d.jobs.length ? [...d.jobs].reverse().map((job) => <JobRow key={job.id} job={job} />) : <div className="empty" role="status">{t('download.list.empty')}</div>}
        </div>
        <div className="download-toolbar">
          <ActionButton name="download_clear" disabled={!finished}>
            {t('download.clear')}
          </ActionButton>
        </div>
      </Card>

      <SectionTitle>{t('sec.download.lanes')}</SectionTitle>
      <Card>
        <Row
          title={t('download.lanes.title')}
          desc={t('download.lanes.desc') + (d.lanes_installed ? '' : ` ${t('download.lanes.brew')}`)}
          state={d.lanes_installed ? 'ready' : 'missing'}
        >
          <div className="download-folder">
            <span className="download-state">{t(d.lanes_running ? 'download.lanes.running' : d.lanes_installed ? 'download.lanes.installed' : 'download.lanes.missing')}</span>
            <ActionButton name="lanes_open" hidden={!d.lanes_installed}>
              {t('download.lanes.open')}
            </ActionButton>
            <ActionButton name="lanes_get" icon="arrow.down.circle.fill" hidden={d.lanes_installed}>
              {t('download.lanes.get')}
            </ActionButton>
          </div>
        </Row>
        <ToggleRow path="download.lanes" title={t('download.lanes.notch.title')} desc={t('download.lanes.notch.desc')} />
      </Card>
    </>
  );
}
