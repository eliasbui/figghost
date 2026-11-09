import { useEffect, useRef } from 'react';
import { useApp } from '@/stores/app';
import { mountNetworkCanvas } from './canvas/mount';
import './network.css';

/** The diagram of Network in a window of its own. It is drawn by hand (canvas/mount.ts) and built again when the language
 *  changes, since its words are written once. */
export default function NetworkCanvas() {
  const host = useRef<HTMLDivElement>(null);
  const lang = useApp((s) => s.lang);
  useEffect(() => {
    document.documentElement.classList.add('canvas-mode');
    const el = host.current;
    if (!el) return;
    return mountNetworkCanvas(el);
  }, [lang]);
  return <div ref={host} style={{ display: 'contents' }} />;
}
