// The AppKit window has no title bar to drag by, so the page asks for the drag: a press on a drag region (the same
// `data-tauri-drag-region` the Tauri window reads) starts it, and a double click on one zooms the window.
const INTERACTIVE = 'button, a, input, select, textarea, label, [role="button"], [role="switch"], [role="slider"], [role="option"]';

function onRegion(event: MouseEvent): boolean {
  const target = event.target instanceof Element ? event.target : null;
  const region = target?.closest('[data-tauri-drag-region]');
  if (!target || !region) return false;
  if (region === target) return true;
  // "deep" takes a press on anything inside, except on what can be pressed itself.
  return region.getAttribute('data-tauri-drag-region') === 'deep' && !target.closest(INTERACTIVE);
}

/** Starts listening; returns how to stop. */
export function startHostDrag(fig: { drag(): void; zoom(): void }): () => void {
  const down = (event: MouseEvent) => {
    if (event.button === 0 && event.detail < 2 && onRegion(event)) fig.drag();
  };
  const twice = (event: MouseEvent) => {
    if (onRegion(event)) fig.zoom();
  };
  document.addEventListener('mousedown', down);
  document.addEventListener('dblclick', twice);
  return () => {
    document.removeEventListener('mousedown', down);
    document.removeEventListener('dblclick', twice);
  };
}
