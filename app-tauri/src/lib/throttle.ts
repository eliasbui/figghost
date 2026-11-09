interface Slot {
  last: number;
  timer: ReturnType<typeof setTimeout> | 0;
  fn: () => void;
}
const slots = new Map<string, Slot>();

/** Runs `fn` at most once every `ms` for a key, with the last call of a burst running at its end, so a slider that
 *  is dragged sends its values at a steady pace and always ends on the one it stopped at. */
export function throttleBy(key: string, ms: number, fn: () => void): void {
  let slot = slots.get(key);
  if (!slot) {
    slot = { last: 0, timer: 0, fn };
    slots.set(key, slot);
  }
  slot.fn = fn;
  const flush = () => {
    const s = slots.get(key);
    if (!s) return;
    s.timer = 0;
    s.last = performance.now();
    s.fn();
  };
  const wait = slot.last + ms - performance.now();
  if (wait <= 0) {
    if (slot.timer) clearTimeout(slot.timer);
    flush();
  } else if (!slot.timer) {
    slot.timer = setTimeout(flush, wait);
  }
}
