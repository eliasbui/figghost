import { lazy, Suspense, useEffect, useState } from 'react';
import { MotionConfig } from 'motion/react';
import { createBridge, insideTauri } from '@/bridge';
import { ConfirmHost, Toast } from '@/components/macos/Dialogs';
import { useRecorder } from '@/components/macos/Hotkey';
import { startSheen } from '@/lib/sheen';
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

/** The facts about the display and the window that the style sheet follows: the classes of the root element. The app says
 *  what the system is set to; the browser says it too, and either is enough. */
function useDisplayClasses(): void {
  const env = useApp((s) => s.state.env);
  const lang = useApp((s) => s.lang);
  const transparency = useMedia('(prefers-reduced-transparency: reduce)');
  const motion = useMedia('(prefers-reduced-motion: reduce)');
  const contrast = useMedia('(prefers-contrast: more)');
  useEffect(() => {
    const list = document.documentElement.classList;
    const reduceTransparency = !!env.reduce_transparency || transparency;
    list.toggle('reduce-transparency', reduceTransparency);
    list.toggle('reduce-motion', !!env.reduce_motion || motion);
    list.toggle('increase-contrast', !!env.increase_contrast || contrast);
    list.toggle('glass', !!env.glass && !reduceTransparency);
  }, [env, transparency, motion, contrast]);
  useEffect(() => {
    startSheen();
    // In a browser, outside the app, a wallpaper stands in for the desktop, and ?theme=dark|light picks a theme.
    const list = document.documentElement.classList;
    if (!insideTauri()) list.add('preview');
    const theme = new URLSearchParams(window.location.search).get('theme');
    if (theme === 'dark' || theme === 'light') list.add(theme);
  }, []);
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
  useEffect(() => {
    // In a window that is not in front, the selection turns grey, as in any Mac sidebar.
    const list = document.documentElement.classList;
    const away = () => list.add('inactive');
    const back = () => list.remove('inactive');
    // This is an app, not a web page: no browser menu, except where the person may want to copy text.
    const menu = (e: MouseEvent) => {
      if (!(e.target instanceof Element) || !e.target.closest('.code')) e.preventDefault();
    };
    window.addEventListener('blur', away);
    window.addEventListener('focus', back);
    document.addEventListener('contextmenu', menu);
    return () => {
      window.removeEventListener('blur', away);
      window.removeEventListener('focus', back);
      document.removeEventListener('contextmenu', menu);
    };
  }, []);
}

/** A feature that draws a window of its own gets this page told which canvas to show (`?canvas=network`); the window has
 *  no sidebar and no toolbar, and keeps its state and its language like any other. */
const CANVAS = (() => {
  const id = new URLSearchParams(window.location.search).get('canvas');
  if (!id) return null;
  const load = canvasLoader(id);
  if (!load) console.error(`no canvas is named ${id}`);
  return { Page: load ? lazy(load) : null };
})();

export default function App() {
  useBridge();
  useDisplayClasses();
  useRecorder();
  const ready = useApp((s) => s.ready);
  // Nothing is drawn until the first state arrives, so that no control shows a value that is only a sample.
  if (!ready) return null;
  if (CANVAS) {
    const { Page } = CANVAS;
    return (
      <MotionConfig reducedMotion="user">
        {Page ? (
          <Suspense fallback={null}>
            <Page />
          </Suspense>
        ) : null}
        <Toast />
      </MotionConfig>
    );
  }
  return (
    <MotionConfig reducedMotion="user">
      <div id="app">
        <div className="dragzone" data-tauri-drag-region="" />
        <Sidebar />
        <main id="main">
          <PageHost />
          <div className="scroll-edge" aria-hidden="true" />
          <Toolbar />
        </main>
      </div>
      <Toast />
      <ConfirmHost />
    </MotionConfig>
  );
}
