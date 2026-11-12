import { useState } from 'react';
import type { KeyboardEvent } from 'react';
import { hasKey } from '@/i18n';
import { useT } from '@/stores/hooks';
import type { TFunction } from '@/stores/hooks';
import './notch.css';

export interface PreviewTab {
  id: string;
  title: string;
  on: boolean;
  waiting: boolean;
}

export interface PreviewNotch {
  enabled: boolean;
  hardware: boolean;
  material: string;
  motion: string;
  shadow: boolean;
  rest: string;
  rest_clock: boolean;
  width: number;
  height: number;
  rest_width: number;
  rest_height: number;
  pages: PreviewTab[];
}

const title = (t: TFunction, tab: PreviewTab) => (hasKey(`tab.${tab.id}`) ? t(`tab.${tab.id}`) : tab.title);

/**
 * A small live screen that draws the settings of the notch: a hardware cutout or a floating pill, the size style,
 * the material, the shadow, the date in the closed shape. It opens on hover and focus, and stays open when pressed.
 * A real screen is drawn at about three quarters of its size here, so the shapes keep their proportions.
 */
export function NotchPreview({ notch }: { notch: PreviewNotch }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const hw = !!notch.hardware;
  const rest: [number, number] = hw ? [180, 32] : [notch.rest_width || 200, notch.rest_height || 24];
  const shown = notch.pages.filter((p) => p.on && !p.waiting);
  const openWidth = (notch.width || 560) * 0.64;
  // As many tab names as the open shape has room for, then how many more there are.
  const room = Math.max(3, Math.min(7, Math.floor((openWidth - 90) / 70)));
  const names = shown.length > room ? shown.slice(0, room - 1) : shown;

  const vars = {
    '--rw': `${Math.round(rest[0] * 0.75)}px`,
    '--rh': `${Math.round(Math.max(14, rest[1] * 0.92))}px`,
    '--ow': `${Math.round(openWidth)}px`,
    '--oh': `${Math.round(Math.min(160, (notch.height || 440) * 0.32))}px`,
    '--cut': `${Math.round(180 * 0.75)}px`,
    '--ear': hw && notch.rest_clock ? '54px' : '0px',
  } as React.CSSProperties;

  const toggle = () => setOpen((o) => !o);
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      toggle();
    }
  };
  const cls = [
    'np',
    hw && 'hw',
    notch.rest_clock && 'clock',
    !hw && notch.rest === 'hidden' && !notch.rest_clock && 'shy',
    notch.shadow && 'sh',
    !notch.enabled && 'off',
    open && 'open',
    `mo-${notch.motion}`,
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className={cls} style={vars} role="button" tabIndex={0} aria-pressed={open} aria-label={t('notch.preview')} onClick={toggle} onKeyDown={onKey}>
      <div className="np-win" aria-hidden="true" />
      <div className="np-win b" aria-hidden="true" />
      <div className="np-orb" aria-hidden="true" />
      <div className="np-menubar" aria-hidden="true">
        <div className="np-l">
          <i className="np-dot" />
          <b>Finder</b>
          <span>File</span>
          <span>Edit</span>
          <span>View</span>
        </div>
        <div className="np-r">
          <i className="np-ico" />
          <i className="np-ico" />
          <i className="np-ico wide" />
        </div>
      </div>
      <div className={`np-notch mat-${notch.material}${notch.material === 'black' ? '' : ' rimmed'}`}>
        <div className="np-rest" aria-hidden="true">
          <span>Tue 5</span>
          <span>9:41</span>
        </div>
        <div className="np-body" aria-hidden="true">
          <div className="np-tabs">
            {names.map((p, i) => (
              <span key={p.id} className={i === 0 ? 'on' : undefined}>
                {title(t, p)}
              </span>
            ))}
            {shown.length > names.length ? <span>{`+${shown.length - names.length}`}</span> : null}
          </div>
          <div className="np-line a" />
          <div className="np-line" />
          <div className="np-line" />
        </div>
      </div>
      <div className="np-note">{t('notch.preview')}</div>
    </div>
  );
}
