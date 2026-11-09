import { Fragment, useEffect, useState } from 'react';
import { Button, Keycaps, Segmented, SwitchButton } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { isModifierCode, keyName } from '@/lib/shortcuts';
import { cx } from '@/lib/text';
import { change, send, showToast } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './wheel.css';

type WheelName = 'formats' | 'tools' | 'actions';
interface Wheel {
  enabled: boolean;
  key: string;
  hidden: string[];
}
interface WheelState {
  enabled: boolean;
  formats: Wheel;
  tools: Wheel;
  actions: Wheel;
}

const WHEELS: WheelName[] = ['formats', 'tools', 'actions'];
// The four modifiers, with the glyph of each: shift, command, control, option.
const KEYS: Array<[string, string]> = [
  ['shift', '\u21e7 Shift'],
  ['command', '\u2318 Command'],
  ['control', '\u2303 Control'],
  ['option', '\u2325 Option'],
];
const GROUPS = ['image', 'video', 'audio', 'pdf', 'document', 'any'];
// Every slot of every wheel as group:id, in the order of catalog::all_ids; a unit test keeps the two in step.
const SLOTS: Record<WheelName, string[]> = {
  formats: [
    'image:to_png', 'image:to_jpeg', 'image:to_heic', 'image:to_tiff', 'image:to_avif', 'image:to_gif', 'image:to_bmp', 'image:to_pdf',
    'video:to_mp4', 'video:to_mov', 'video:to_gif', 'video:to_m4a',
    'audio:to_m4a', 'audio:to_wav', 'audio:to_aiff', 'audio:to_flac', 'audio:to_caf', 'audio:to_aac',
    'pdf:to_png', 'pdf:to_jpeg',
    'document:to_docx', 'document:to_doc', 'document:to_rtf', 'document:to_odt', 'document:to_html', 'document:to_txt', 'document:to_pdf',
    'any:zip', 'any:tar', 'any:tgz',
  ],
  tools: [
    'image:compress', 'image:half', 'image:quarter', 'image:strip_metadata', 'image:remove_background', 'image:copy_text', 'image:annotate',
    'video:compress_1080p', 'video:compress_720p', 'video:compress_540p', 'video:strip_metadata',
    'audio:compress',
    'pdf:pdf_merge', 'pdf:pdf_split',
  ],
  actions: ['any:copy_path', 'any:copy', 'any:airdrop', 'any:terminal', 'any:reveal', 'any:float'],
};

const split = (token: string) => token.split(':');
const uniqueIds = (wheel: WheelName) => [...new Set(SLOTS[wheel].map((s) => split(s)[1]))];
// The names of formats and archives are written the same in every language.
const isFormat = (id: string) => id.startsWith('to_') || ['zip', 'tar', 'tgz'].includes(id);

/** Another wheel that is on and uses this key. */
const takenBy = (state: WheelState, wheel: WheelName, key: string) => WHEELS.find((o) => o !== wheel && state[o].enabled && state[o].key === key);

/** Any other key: the button records the next one pressed, as a shortcut field does, but alone, without a modifier.
 *  Escape gives up, since it is the key that ends a drag; a key no shortcut knows says so. */
function OtherKey({ wheel, state }: { wheel: WheelName; state: WheelState }) {
  const t = useT();
  const [recording, setRecording] = useState(false);
  const current = state[wheel].key;
  const custom = !KEYS.some(([value]) => value === current);

  useEffect(() => {
    if (!recording) return;
    const stop = () => {
      setRecording(false);
      send({ t: 'recording', on: false });
    };
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      // A modifier alone is one of the four choices beside the button.
      if (isModifierCode(e.code)) return;
      if (e.code === 'Escape') {
        stop();
        return;
      }
      const key = keyName(e.code);
      if (!key) {
        showToast(t('fault.unsupported_key'));
        return;
      }
      stop();
      if (key === current) return;
      if (state[wheel].enabled && takenBy(state, wheel, key)) {
        showToast(t('fault.wheel_key_taken'));
        return;
      }
      change(`wheel.${wheel}.key`, key);
    };
    const onAway = (e: MouseEvent) => {
      if (!(e.target instanceof Element) || !e.target.closest('.wheel-other')) stop();
    };
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('mousedown', onAway, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('mousedown', onAway, true);
    };
  }, [recording, current, wheel, state, t]);

  return (
    <Button
      size="sm"
      className={cx('wheel-other', custom && !recording && 'on', recording && 'recording')}
      aria-label={t('wheel.key.other') + (custom ? `: ${current}` : '')}
      onClick={() => {
        if (!recording) {
          // Global shortcuts let go while a key is recorded, so the key reaches the page.
          send({ t: 'recording', on: true });
        } else {
          send({ t: 'recording', on: false });
        }
        setRecording(!recording);
      }}
    >
      {recording ? t('wheel.key.press') : custom ? <Keycaps shortcut={current} /> : t('wheel.key.other')}
    </Button>
  );
}

function Pick({ wheel, id, hidden }: { wheel: WheelName; id: string; hidden: string[] }) {
  const t = useT();
  const title = isFormat(id) ? (id.startsWith('to_') ? id.slice(3) : id).toUpperCase() : t(`wheel.slot.${id}`);
  return (
    <button
      type="button"
      className="wheel-pick"
      data-id={id}
      aria-pressed={!hidden.includes(id)}
      onClick={() => {
        const set = new Set(hidden);
        if (set.has(id)) set.delete(id);
        else set.add(id);
        change(`wheel.${wheel}.hidden`, uniqueIds(wheel).filter((i) => set.has(i)));
      }}
    >
      <span className="wheel-name">{title}</span>
    </button>
  );
}

/** The slots a wheel can show, by kind of file, each a button that shows or hides it. */
function Slots({ wheel, hidden, on }: { wheel: WheelName; hidden: string[]; on: boolean }) {
  const t = useT();
  return (
    <div className={cx('wheel-body', !on && 'dim')}>
      {GROUPS.map((group) => {
        const ids = SLOTS[wheel].filter((s) => split(s)[0] === group).map((s) => split(s)[1]);
        if (!ids.length) return null;
        return (
          <Fragment key={group}>
            <div className="wheel-group">{t(`wheel.group.${group}`)}</div>
            <div className="wheel-picks" role="group" aria-label={t(`wheel.group.${group}`)}>
              {ids.map((id) => (
                <Pick key={id} wheel={wheel} id={id} hidden={hidden} />
              ))}
            </div>
          </Fragment>
        );
      })}
      <div className="wheel-foot">
        <div className="row-desc" role="status">
          {t('wheel.shown', { n: uniqueIds(wheel).length - hidden.length, max: uniqueIds(wheel).length })}
        </div>
        <Button small className="wheel-showall" disabled={hidden.length === 0} onClick={() => change(`wheel.${wheel}.hidden`, [])}>
          {t('wheel.showall')}
        </Button>
      </div>
    </div>
  );
}

function WheelCard({ wheel, state }: { wheel: WheelName; state: WheelState }) {
  const t = useT();
  const w = state[wheel];
  const pickKey = (key: string) => {
    if (key === w.key) return;
    if (w.enabled && takenBy(state, wheel, key)) {
      showToast(t('fault.wheel_key_taken'));
      return;
    }
    change(`wheel.${wheel}.key`, key);
  };
  return (
    <section className="card">
      <Row title={t(`wheel.${wheel}.title`)} desc={t(`wheel.${wheel}.desc`)}>
        <SwitchButton
          checked={!!w.enabled}
          label={t(`wheel.${wheel}.title`)}
          onToggle={() => {
            const next = !w.enabled;
            // The app refuses it too; saying so here keeps the switch from flickering on and back.
            if (next && takenBy(state, wheel, w.key)) {
              showToast(t('fault.wheel_key_taken'));
              return;
            }
            change(`wheel.${wheel}.enabled`, next);
          }}
        />
      </Row>
      <Row title={t('wheel.key.title')} desc={t('wheel.key.desc')} when={w.enabled}>
        <div className="wheel-key">
          <Segmented label={t('wheel.key.title')} value={w.key} options={KEYS.map(([value, label]) => ({ value, label }))} onPick={pickKey} />
          <OtherKey wheel={wheel} state={state} />
        </div>
      </Row>
      <Row title={t('wheel.slots.title')} desc={t('wheel.slots.desc')} block when={w.enabled} />
      <Slots wheel={wheel} hidden={w.hidden} on={w.enabled} />
    </section>
  );
}

/** The drag wheel: while files are dragged, tapping a key opens a wheel of slots around the pointer; dropping the files on a
 *  slot runs it. There are three wheels (convert formats, tools, actions); each has a switch, the key that opens it and the
 *  choice of the slots it shows. */
export default function WheelPage() {
  const state = useSection<WheelState>('wheel');
  return (
    <>
      {WHEELS.map((wheel) => (
        <WheelCard key={wheel} wheel={wheel} state={state} />
      ))}
    </>
  );
}
