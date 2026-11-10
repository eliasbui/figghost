import { MusicNote, Pause, Play, SkipBack, SkipForward } from '@phosphor-icons/react';
import { useEffect, useRef, useState } from 'react';
import type { PointerEvent as ReactPointerEvent, ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { cn } from '@/lib/cn';
import '@/styles/surface.css';
import { send, startWindow, useHost } from '../host';
import { FIRST_VIEW, clock, isStyle, next, position, seekTo } from './logic';
import type { State, Style, View } from './logic';

const ctl = (name: string) => send({ t: 'cmd', name });

/** The song's place, a second at a time or finer: its own component, so that only the bar is drawn again while it moves. */
function Progress({ view, className, withTimes }: { view: View; className?: string; withTimes: boolean }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(id);
  }, []);
  const track = view.track;
  if (!track) return null;
  const shown = position(track, view.at, now);
  const percent = track.dur ? (100 * shown) / track.dur : 0;
  return (
    <>
      <div
        role="slider"
        aria-label={view.labels.seek ?? 'Position'}
        aria-valuemin={0}
        aria-valuemax={track.dur ?? 0}
        aria-valuenow={Math.round(shown)}
        className={cn('group/bar relative flex h-[18px] items-center', className)}
        onClick={(event) => {
          if (!track.dur) return;
          const box = event.currentTarget.getBoundingClientRect();
          send({ t: 'seek', ms: seekTo(track, (event.clientX - box.left) / box.width) });
        }}
      >
        <i className="block h-1 w-full overflow-hidden rounded-sm bg-white/26 group-hover/bar:h-1.5">
          <b className="block h-full rounded-sm bg-white" style={{ width: `${percent}%` }} />
        </i>
      </div>
      {withTimes ? (
        <div className={cn('-mt-px flex justify-between text-[10.5px] text-white/36 tabular-nums', className)}>
          <span>{clock(shown)}</span>
          <span>{track.dur ? clock(track.dur) : ''}</span>
        </div>
      ) : null}
    </>
  );
}

function Round({ label, onClick, play, small, hidden, children }: { label: string; onClick: () => void; play?: boolean; small?: boolean; hidden?: boolean; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className={cn(
        'grid place-items-center rounded-full text-white transition-[background-color,transform] duration-100 active:scale-90 [&_svg]:fill-current',
        play ? 'bg-white/20 hover:bg-white/30' : 'hover:bg-white/16',
        play ? (small ? 'size-[34px]' : 'size-[42px]') : small ? 'size-[30px]' : 'size-[34px]',
        play ? '[&_svg]:size-[22px]' : '[&_svg]:size-[18px]',
        hidden && 'hidden',
      )}
    >
      {children}
    </button>
  );
}

const ART: Record<Style, string> = {
  card: 'h-full max-h-[104px] rounded-[14px]',
  cover: 'size-full aspect-auto rounded-[var(--r)] shadow-none',
  bar: 'h-full rounded-full',
  vinyl: 'h-auto w-[62%] animate-[spin_9s_linear_infinite] rounded-full shadow-[0_10px_26px_rgb(0_0_0/0.4),inset_0_0_0_0.5px_rgb(255_255_255/0.2)] after:absolute after:top-1/2 after:left-1/2 after:size-[16%] after:-translate-x-1/2 after:-translate-y-1/2 after:rounded-full after:bg-[#0c0c10] after:shadow-[0_0_0_3px_rgb(255_255_255/0.16)] after:content-[""]',
  minimal: 'hidden',
};
const ROOT: Record<Style, string> = {
  card: 'items-center gap-3.5',
  cover: 'p-0',
  bar: 'items-center gap-2.5 py-2 pr-3.5 pl-2',
  vinyl: 'flex-col items-center gap-2.5 px-4 pt-[18px] pb-3.5',
  minimal: 'items-center gap-3 px-4 py-3',
};

function Song({ view }: { view: View }) {
  const { style, track, art, labels } = view;
  if (!track) return null;
  const small = style === 'bar';
  const playing = !!track.playing;
  const cover = (
    <div
      className={cn('relative grid aspect-square flex-none place-items-center overflow-hidden rounded-[14px] bg-white/14 bg-cover bg-center text-white/36 shadow-[0_6px_18px_rgb(0_0_0/0.28),inset_0_0_0_0.5px_rgb(255_255_255/0.22)]', ART[style], style === 'vinyl' && !playing && '[animation-play-state:paused]')}
      style={art ? { backgroundImage: `url("${art}")` } : undefined}
    >
      {art ? null : <MusicNote className="size-1/3 fill-current" weight="fill" />}
    </div>
  );
  const text = (
    <div className={cn('min-w-0', (style === 'bar' || style === 'minimal') && 'flex-1', style === 'vinyl' && 'w-full text-center')}>
      <div className={cn('truncate text-[15px] font-[650] tracking-[-0.01em]', small && 'text-[13px]', style === 'minimal' && 'text-base')}>{track.title}</div>
      <div className="truncate text-xs text-white/66">{[track.artist, track.album].filter(Boolean).join(' - ') || track.app || ''}</div>
    </div>
  );
  const skip = style !== 'minimal';
  const buttons = (
    <div className={cn('flex items-center gap-1', style === 'card' && 'mt-0.5 -ml-1.5', style === 'cover' && 'mt-0.5 translate-y-1.5 justify-center opacity-0 transition-[opacity,transform] duration-200 group-hover/widget:translate-y-0 group-hover/widget:opacity-100', style === 'vinyl' && 'mt-auto')}>
      <Round label={labels.prev ?? 'Previous'} small={small} hidden={!skip} onClick={() => ctl('prev')}>
        <SkipBack weight="fill" />
      </Round>
      <Round label={labels.play ?? 'Play or pause'} play small={small} onClick={() => ctl('play')}>
        {playing ? <Pause weight="fill" /> : <Play weight="fill" />}
      </Round>
      <Round label={labels.next ?? 'Next'} small={small} hidden={!skip} onClick={() => ctl('next')}>
        <SkipForward weight="fill" />
      </Round>
    </div>
  );
  if (style === 'cover') {
    return (
      <>
        {cover}
        <div className="absolute inset-x-0 bottom-0 flex flex-col gap-0.5 rounded-b-[var(--r)] bg-gradient-to-b from-transparent to-black/60 px-4 pt-10 pb-3">
          {text}
          <Progress view={view} className="mt-1" withTimes={false} />
          {buttons}
        </div>
      </>
    );
  }
  if (style === 'bar') return <>{cover}{text}{buttons}</>;
  if (style === 'minimal') return <>{text}{buttons}</>;
  if (style === 'vinyl') {
    return (
      <>
        {cover}
        {text}
        <Progress view={view} className="w-full" withTimes />
        {buttons}
      </>
    );
  }
  return (
    <>
      {cover}
      <div className="flex min-w-0 flex-1 flex-col justify-center gap-0.5">
        {text}
        <Progress view={view} className="w-full" withTimes />
        {buttons}
      </div>
    </>
  );
}

/** The corner that resizes the widget: the screen's own coordinates keep steady while the window moves under the pointer. */
function Grip() {
  const pull = useRef<{ x: number; y: number; w: number; h: number } | null>(null);
  const queued = useRef(false);
  const down = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    pull.current = { x: event.screenX, y: event.screenY, w: window.innerWidth, h: window.innerHeight };
    event.preventDefault();
  };
  const move = (event: ReactPointerEvent<HTMLDivElement>) => {
    const start = pull.current;
    if (!start || queued.current) return;
    queued.current = true;
    const [x, y] = [event.screenX, event.screenY];
    requestAnimationFrame(() => {
      queued.current = false;
      if (pull.current) send({ t: 'grip', w: start.w + x - start.x, h: start.h + y - start.y });
    });
  };
  return (
    <div
      role="button"
      aria-label="Resize"
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={() => {
        pull.current = null;
      }}
      className="absolute right-1 bottom-1 z-[3] size-[18px] cursor-nwse-resize bg-[linear-gradient(135deg,transparent_52%,rgb(255_255_255/0.36)_52%,rgb(255_255_255/0.36)_60%,transparent_60%,transparent_72%,rgb(255_255_255/0.36)_72%,rgb(255_255_255/0.36)_80%,transparent_80%)] opacity-0 transition-opacity duration-150 group-hover/widget:opacity-100"
    />
  );
}

function Widget() {
  const [view, setView] = useState<View>(FIRST_VIEW);
  useHost<State>((message) => {
    if (message.t === 'state') setView((before) => next(before, message, Date.now()));
  });
  const { style, track, art, ambient, radius, labels } = view;
  return (
    <div
      data-tauri-drag-region="deep"
      data-style={isStyle(style) ? style : 'card'}
      className="group/widget relative h-full overflow-hidden rounded-[var(--r)] text-[13px] leading-[1.3] text-white"
      style={{ '--r': `${radius}px` } as React.CSSProperties}
    >
      {/* The blurred cover behind everything: light that takes the colour of the music. */}
      <div aria-hidden="true" className={cn('pointer-events-none absolute -inset-[30px] z-0 bg-cover bg-center blur-[34px] saturate-[1.7] transition-opacity duration-[600ms]', ambient && art ? 'opacity-55' : 'opacity-0')} style={art ? { backgroundImage: `url("${art}")` } : undefined} />
      <div data-tauri-drag-region="deep" className={cn('pointer-events-none relative z-[1] flex size-full p-3.5 *:pointer-events-auto', ROOT[style])}>
        {track ? <Song view={view} /> : <div className="m-auto text-center text-white/66">{labels.idle ?? 'Nothing is playing'}</div>}
      </div>
      <Grip />
    </div>
  );
}

startWindow();
createRoot(document.getElementById('root')!).render(<Widget />);
