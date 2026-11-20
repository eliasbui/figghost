import { useEffect, useState } from 'react';
import { CheckboxInput } from '@astryxdesign/core/CheckboxInput';
import { ActionButton, ActionIcon, Button } from '@/ui/controls';
import { Card, Row, SectionTitle } from '@/ui/layout';
import { ToggleRow } from '@/ui/rows';
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

  // A software that is no longer on the list is no longer chosen.
  useEffect(() => {
    setChosen((now) => (now.every((k) => list.some((x) => x.key === k)) ? now : now.filter((k) => list.some((x) => x.key === k))));
  }, [u.list]);

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
          <div className="upd-all">
            <CheckboxInput
              label={t('updates.pick.all')}
              value={list.length > 0 && chosen.length === list.length ? true : chosen.length > 0 ? 'indeterminate' : false}
              isDisabled={!list.length || u.busy}
              onChange={(checked) => setChosen(checked ? list.map((x) => x.key) : [])}
            />
          </div>
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
              <div
                className="upd-item"
                key={x.key}
                onClick={(e) => {
                  if (!(e.target as HTMLElement).closest('label, input')) toggle(x.key, !chosen.includes(x.key));
                }}
              >
                <CheckboxInput label={x.name} isLabelHidden value={chosen.includes(x.key)} onChange={(checked) => toggle(x.key, checked)} />
                <span className="upd-text">
                  <span className="upd-name">{x.name}</span>
                  <span className="upd-source">{x.source}</span>
                </span>
                <span className="upd-versions">
                  {x.current ? <span className="upd-old">{x.current}</span> : null}
                  {x.current ? <span className="upd-arrow">-&gt;</span> : null}
                  <span className="upd-new">{x.latest}</span>
                </span>
              </div>
            ))
          ) : (
            <div className="empty">{t(on ? 'updates.empty' : 'updates.off')}</div>
          )}
        </div>
      </Card>
    </>
  );
}
