import { useEffect, useRef, useState } from 'react';

/** A value a control keeps as its own while it is being moved: set it on every move, and it lets go on `null` or, if the
 *  control never says it was let go (a key does not), shortly after the last move. */
export function useHeld<T>(letGoAfter = 400): [T | null, (value: T | null) => void] {
  const [held, setHeld] = useState<T | null>(null);
  const timer = useRef(0);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const hold = (value: T | null) => {
    window.clearTimeout(timer.current);
    setHeld(value);
    if (value !== null) timer.current = window.setTimeout(() => setHeld(null), letGoAfter);
  };
  return [held, hold];
}
