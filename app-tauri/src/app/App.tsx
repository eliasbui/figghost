import { lazy, Suspense, useEffect, useState } from 'react';
import { Theme } from '@astryxdesign/core/theme';
import { neutralTheme } from '@astryxdesign/theme-neutral/built';
import { createBridge } from '@/bridge';
import { ConfirmHost, Toast } from '@/ui/dialogs';
import { useRecorder } from '@/ui/hotkey';
import { startHostDrag } from '@/lib/drag';
import { canvasLoader } from '@/pages/canvases';
import { connect, useApp } from '@/stores/app';
import { PageHost } from './PageHost';
import { Sidebar } from './Sidebar';
import { Toolbar } from './Toolbar';

/** Listens to the app while the window is open, and tells it the page is ready. */
function useBridge(): void {
  useEffect(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    createBridge()
      // A page that was taken down before its bridge was made does not connect: two connections would share `bridge`, and the
      // one that ends would leave the other with none (StrictMode runs an effect twice in development).
      .then((bridge) => (gone ? null : connect(bridge)))
      .then((off) => {
        if (!off) return;
        if (gone) off();
        else stop = off;
      })
      .catch((error) => console.error(error));
    return () => {
      gone = true;
      stop?.();
    };
  }, []);
}

/** Whether a media query holds now, and again whenever it changes. */
function useMedia(query: string): boolean {
  const [on, setOn] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const media = window.matchMedia(query);
    const update = () => setOn(media.matches);
    media.addEventListener('change', update);
    update();
    return () => media.removeEventListener('change', update);
  }, [query]);
  return on;
}

/** The facts about the window that the style sheet follows: whether the system asks for less motion (the app says what the system
 *  is set to; the browser says it too, and either is enough), the language of the page, and the menu of the browser. */
function useDisplayClasses(): void {
  const env = useApp((s) => s.state.env);
  const lang = useApp((s) => s.lang);
  const motion = useMedia('(prefers-reduced-motion: reduce)');
  useEffect(() => {
    document.documentElement.classList.toggle('reduce-motion', !!env.reduce_motion || motion);
  }, [env, motion]);
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
  useEffect(() => {
    // This is an app, not a web page: no browser menu, except where the person may want to copy text.
    const menu = (e: MouseEvent) => {
      if (!(e.target instanceof Element) || !e.target.closest('.code')) e.preventDefault();
    };
    document.addEventListener('contextmenu', menu);
    return () => document.removeEventListener('contextmenu', menu);
  }, []);
}

/** A feature that draws a window of its own gets this page told which canvas to show (`?canvas=network`); the window has
 *  no sidebar and no toolbar, and keeps its state and its language like any other. */
const CANVAS = (() => {
  const id = window.__canvas ?? new URLSearchParams(window.location.search).get('canvas');
  if (!id) return null;
  const load = canvasLoader(id);
  if (!load) console.error(`no canvas is named ${id}`);
  return { Page: load ? lazy(load) : null };
})();

/** The theme of Astryx follows the system; `?theme=dark|light` pins it, for looking at a page in a browser. */
const PINNED = new URLSearchParams(window.location.search).get('theme');
const MODE = PINNED === 'dark' || PINNED === 'light' ? PINNED : 'system';

export default function App() {
  useBridge();
  useEffect(() => (window.fig ? startHostDrag(window.fig) : undefined), []);
  useDisplayClasses();
  useRecorder();
  const ready = useApp((s) => s.ready);
  // Nothing is drawn until the first state arrives, so that no control shows a value that is only a sample.
  if (!ready) return null;
  if (CANVAS) {
    const { Page } = CANVAS;
    return (
      <Theme theme={neutralTheme} mode={MODE}>
        {Page ? (
          <Suspense fallback={null}>
            <Page />
          </Suspense>
        ) : null}
        <Toast />
      </Theme>
    );
  }
  return (
    <Theme theme={neutralTheme} mode={MODE}>
      <div id="app">
        <div className="dragzone" data-tauri-drag-region="" />
        <Sidebar />
        <main id="main">
          <PageHost />
          <Toolbar />
        </main>
      </div>
      <Toast />
      <ConfirmHost />
    </Theme>
  );
}
