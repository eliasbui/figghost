// The light that follows the pointer over a piece of glass: one listener for the window writes where the pointer is, as
// --mx and --my in pixels from the corner of the nearest `.glass` or `.gl`; the style sheet does the rest.
let started = false;
let last: HTMLElement | null = null;

export function startSheen(): void {
  if (started || typeof document === 'undefined') return;
  started = true;
  document.addEventListener(
    'pointermove',
    (e) => {
      if (e.pointerType === 'touch') return;
      const target = e.target instanceof Element ? e.target.closest<HTMLElement>('.glass, .gl') : null;
      if (last && last !== target) {
        last.style.removeProperty('--mx');
        last.style.removeProperty('--my');
      }
      last = target;
      if (!target) return;
      const box = target.getBoundingClientRect();
      target.style.setProperty('--mx', `${Math.round(e.clientX - box.left)}px`);
      target.style.setProperty('--my', `${Math.round(e.clientY - box.top)}px`);
    },
    { passive: true },
  );
}
