import { useEffect, useRef } from 'react';
import type { RefObject } from 'react';
import { cssColor, cssVar } from '@/lib/dom';
import { tr } from '@/stores/app';
import { useT } from '@/stores/hooks';

const BARS = 60;
const IDLE_MS = 500;

/** A strip of text to scroll with the wheel, and beside it what the scrolling does: its speed, its peak, how many frames
 *  it took and the biggest jump between two. The numbers are written to the page directly, since they change every frame. */
export function ScrollPad() {
  const t = useT();
  const pad = useRef<HTMLDivElement>(null);
  const meter = useRef<HTMLCanvasElement>(null);
  const speed = useRef<HTMLElement>(null);
  const peak = useRef<HTMLElement>(null);
  const frames = useRef<HTMLElement>(null);
  const jump = useRef<HTMLElement>(null);

  useEffect(() => {
    const padEl = pad.current;
    const canvas = meter.current;
    const ctx = canvas?.getContext('2d');
    if (!padEl || !canvas || !ctx) return;

    let last = { top: 0, at: 0 };
    let running = false;
    let idleSince = 0;
    let handle = 0;
    let deltas: number[] = [];
    const stats = { peak: 0, frames: 0, jump: 0 };

    const paint = () => {
      const dpr = window.devicePixelRatio || 1;
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (!w) return;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);
      if (!deltas.length) {
        // Waiting for a scroll: a dashed baseline and a short hint.
        ctx.strokeStyle = cssVar('--text-3');
        ctx.setLineDash([4, 4]);
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(10, h - 14);
        ctx.lineTo(w - 10, h - 14);
        ctx.stroke();
        ctx.setLineDash([]);
        ctx.fillStyle = cssVar('--text-2');
        ctx.font = `12px ${cssVar('--font')}`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(tr('pad.idle'), w / 2, h / 2 - 6);
        return;
      }
      const top = Math.max(20, ...deltas);
      const bar = w / BARS;
      const tint = cssColor('--tint');
      deltas.forEach((d, i) => {
        const height = (d / top) * (h - 8);
        ctx.fillStyle = tint;
        ctx.globalAlpha = 0.35 + 0.65 * (i / deltas.length);
        ctx.fillRect(i * bar + 1, h - 4 - height, Math.max(1, bar - 2), height);
      });
      ctx.globalAlpha = 1;
    };

    const write = (ref: RefObject<HTMLElement | null>, text: string) => {
      if (ref.current) ref.current.textContent = text;
    };

    const frame = (now: number) => {
      const dt = Math.max(1, now - last.at) / 1000;
      const delta = Math.abs(padEl.scrollTop - last.top);
      last = { top: padEl.scrollTop, at: now };
      if (delta > 0) {
        idleSince = now;
        deltas.push(delta);
        if (deltas.length > BARS) deltas.shift();
        stats.frames++;
        stats.jump = Math.max(stats.jump, delta);
        const current = delta / dt;
        stats.peak = Math.max(stats.peak, current);
        write(speed, `${Math.round(current)} px/s`);
      }
      write(peak, `${Math.round(stats.peak)} px/s`);
      write(frames, String(stats.frames));
      write(jump, `${Math.round(stats.jump)} px`);
      paint();
      if (now - idleSince < IDLE_MS) handle = requestAnimationFrame(frame);
      else running = false;
    };

    const onScroll = () => {
      if (running) return;
      // A new gesture starts after a pause: begin the statistics again.
      deltas = [];
      stats.peak = 0;
      stats.frames = 0;
      stats.jump = 0;
      running = true;
      idleSince = performance.now();
      last = { top: padEl.scrollTop, at: performance.now() };
      handle = requestAnimationFrame(frame);
    };

    padEl.addEventListener('scroll', onScroll);
    const observer = new ResizeObserver(paint);
    observer.observe(canvas);
    paint();
    return () => {
      padEl.removeEventListener('scroll', onScroll);
      observer.disconnect();
      cancelAnimationFrame(handle);
    };
  }, []);

  const stat = (label: string, ref: RefObject<HTMLElement | null>) => (
    <div className="stat">
      <span>{label}</span>
      <b ref={ref}>0</b>
    </div>
  );

  return (
    <section className="card">
      <div className="pad-wrap">
        <div className="pad" ref={pad} tabIndex={0} role="region" aria-label={t('pad.hint')}>
          {Array.from({ length: 150 }, (_, i) => (
            <div className="pad-line" key={i}>
              {t('pad.line', { n: i + 1 })}
            </div>
          ))}
        </div>
        <div className="meter">
          <canvas ref={meter} role="img" aria-label={t('pad.speed')} />
          {stat(t('pad.speed'), speed)}
          {stat(t('pad.peak'), peak)}
          {stat(t('pad.frames'), frames)}
          {stat(t('pad.jump'), jump)}
          <div className="row-desc">{t('pad.hint')}</div>
        </div>
      </div>
    </section>
  );
}
