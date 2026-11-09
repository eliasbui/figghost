import { useEffect, useRef, useState } from 'react';
import { ActionButton, ActionIcon, Button, ToggleRow } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './updates.css';

interface Update {
  key: string;
  name: string;
  source: string;
  current?: string;
  latest: string;
}
interface UpdatesState {
  enabled: boolean;
  busy: boolean;
  count: number;
  list?: Update[];
}

/** Updates: a tab of the notch with the software that has a newer version, from Homebrew, npm, the App Store, Apple and
 *  the coding agents, ticked and updated from the notch. */
export default function Updates() {
  const t = useT();
  const u = useSection<UpdatesState>('updates');
  const on = u.enabled;
  const list = u.list ?? [];
  const [chosen, setChosen] = useState<string[]>([]);
  const all = useRef<HTMLInputElement>(null);

  // A software that is no longer on the list is no longer chosen.
  useEffect(() => {
    setChosen((now) => (now.every((k) => list.some((x) => x.key === k)) ? now : now.filter((k) => list.some((x) => x.key === k))));
  }, [u.list]);
  useEffect(() => {
    if (all.current) all.current.indeterminate = chosen.length > 0 && chosen.length < list.length;
  }, [chosen, list.length]);

  const toggle = (key: string, picked: boolean) => setChosen((now) => (picked ? [...now.filter((k) => k !== key), key] : now.filter((k) => k !== key)));
  return (
    <>
      <Card>
        <ToggleRow path="updates.daily" title={t('updates.daily.title')} desc={t('updates.daily.desc')} when={on} />
        <Row title={t('updates.check')} when={on}>
          <div className="row-desc">{t('updates.count', { n: u.count })}</div>
          <ActionIcon name="updates_check" icon="refresh" loading={u.busy} disabled={!on} label={u.busy ? t('updates.checking') : t('updates.check')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.updates.list')}</SectionTitle>
      <Card>
        <div className="upd-toolbar">
          <label className="upd-all">
            <input
              ref={all}
              type="checkbox"
              aria-label={t('updates.pick.all')}
              checked={list.length > 0 && chosen.length === list.length}
              disabled={!list.length || u.busy}
              onChange={(e) => setChosen(e.target.checked ? list.map((x) => x.key) : [])}
            />
            {t('updates.pick.all')}
          </label>
          <Button small primary disabled={!chosen.length || u.busy} onClick={() => send({ t: 'action', name: `updates_run:${chosen.join('\n')}` })}>
            {u.busy ? t('updates.busy') : t('updates.run', { n: chosen.length })}
          </Button>
          <ActionButton name="updates_all" icon="arrow.down.app.fill" disabled={!list.length || u.busy}>
            {t('updates.all')}
          </ActionButton>
        </div>
        <div className="upd-list">
          {list.length ? (
            list.map((x) => (
              <label className="upd-item" key={x.key}>
                <input type="checkbox" aria-label={x.name} checked={chosen.includes(x.key)} onChange={(e) => toggle(x.key, e.target.checked)} />
                <span className="upd-text">
                  <span className="upd-name">{x.name}</span>
                  <span className="upd-source">{x.source}</span>
                </span>
                <span className="upd-versions">
                  {x.current ? <span className="upd-old">{x.current}</span> : null}
                  {x.current ? <span className="upd-arrow">-&gt;</span> : null}
                  <span className="upd-new">{x.latest}</span>
                </span>
              </label>
            ))
          ) : (
            <div className="empty">{t(on ? 'updates.empty' : 'updates.off')}</div>
          )}
        </div>
      </Card>
    </>
  );
}
