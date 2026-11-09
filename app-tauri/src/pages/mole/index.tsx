import { useEffect, useRef } from 'react';
import { AppIcon } from '@/components/macos/Apps';
import { showToast, send, tr } from '@/stores/app';
import { useLang, useSection, useT } from '@/stores/hooks';
import { AnalyzeHead, tintOf } from './Analyze';
import { DockerCard, DockerLists } from './Docker';
import { Health } from './Health';
import { PickList } from './PickList';
import { agoMinutes, size, tilde } from './format';
import type { Item, MoleState } from './types';
import './mole.css';

/** Mole: the health of this Mac and what can be cleaned from it, after the features of Mole (github.com/tw93/Mole) but done
 *  by the app itself. The page scans as it opens and every list can be chosen from and removed together. Every removal is
 *  checked again by the app. */
export default function Mole() {
  const t = useT();
  const lang = useLang();
  const mole = useSection<MoleState>('mole');

  // The page asks the app to scan as it opens.
  useEffect(() => {
    send({ t: 'action', name: 'mole_open' });
  }, []);

  // What a removal did is said once, when the answer has a new number.
  const seen = useRef(mole.result ? mole.result.seq : null);
  useEffect(() => {
    const r = mole.result;
    if (!r || r.seq === seen.current) return;
    seen.current = r.seq;
    if (r.procs) showToast(tr(r.failed ? 'mole.procs.done.partial' : 'mole.procs.done', { n: r.removed, failed: r.failed }));
    else if (r.docker) showToast(tr(r.failed ? 'mole.docker.done.partial' : 'mole.docker.done', { n: r.removed, failed: r.failed }));
    else showToast(tr(r.failed ? 'mole.result.partial' : 'mole.result', { n: r.removed, size: size(r.freed), failed: r.failed }));
  }, [mole.result]);

  const common = { home: mole.home, busy: mole.busy };
  const age = (i: Item) => (i.age == null ? '' : i.age ? t('mole.age', { n: i.age }) : t('mole.age.today'));
  const procs = mole.procs && mole.procs.ok ? mole.procs.items : null;

  return (
    <>
      <Health mole={mole} />
      <PickList {...common} kind="clean" items={mole.clean} detail={(i) => `${t(`mole.group.${i.note}`)} / ${tilde(i.path, mole.home)}`} />
      <PickList {...common} kind="purge" items={mole.purge} detail={(i) => [i.note, age(i)].filter(Boolean).join(' / ')} />
      <PickList {...common} kind="installers" items={mole.installers} detail={(i) => tilde(i.path, mole.home)} />
      <PickList
        {...common}
        kind="apps"
        items={mole.apps}
        filter
        lead={(i) => <AppIcon id={i.note} />}
        detail={(i) => (i.extra ? `${i.note} / ${t('mole.apps.extra', { n: i.extra })}` : i.note)}
      />
      <DockerCard mole={mole} />
      <DockerLists mole={mole} />
      <PickList
        {...common}
        kind="procs"
        ns="procs"
        scan="procs"
        items={procs}
        filter
        detail={(i) =>
          [t(`mole.procs.kind.${i.kind}`), i.ports && i.ports.length ? t('mole.procs.ports', { ports: i.ports.join(', ') }) : '', t('mole.procs.age', { n: agoMinutes(i.minutes ?? 0, lang) })]
            .filter(Boolean)
            .join(' / ')
        }
        actions={[{ verb: 'stop', label: 'stop', danger: true }]}
      />
      <PickList
        {...common}
        kind="analyze"
        items={mole.analyze ? mole.analyze.entries : null}
        head={<AnalyzeHead mole={mole} />}
        detail={() => ''}
        open={(i) => send({ t: 'action', name: `mole_analyze:${i.path}` })}
        tint={tintOf}
      />
    </>
  );
}
