// The messages between the page and the app, as the page of the current app (web/app.js) and its bridge
// (crates/app-ui/src/settings/model.rs, `Message`) speak them, so that one backend can serve both: JSON objects with a
// `t` field that names the kind.

/** Which list an app picked in the page is destined for. */
export type Purpose = 'scroll' | 'ignore' | 'typing';

/** What the page sends. */
export type Outbound =
  | { t: 'ready' }
  | { t: 'set'; path: string; value: unknown }
  | { t: 'app_rule'; id: string; field: 'smooth' | 'reverse'; value: boolean | null }
  | { t: 'app_add'; id: string }
  | { t: 'app_remove'; id: string }
  | { t: 'ignored_add'; id: string }
  | { t: 'ignored_remove'; id: string }
  | { t: 'hotkey'; target: string; value: string }
  /** The page is capturing a key combination: global shortcuts must not fire meanwhile. */
  | { t: 'recording'; on: boolean }
  | { t: 'action'; name: string }
  | { t: 'pick_app'; purpose: Purpose }
  /** The apps that are open now, to offer in the "add app" menu. */
  | { t: 'running'; purpose: Purpose }
  | { t: 'icons'; ids: string[] }
  | { t: 'page'; id: string }
  | { t: 'js_error'; message: string; line?: number };

export interface RunningApp {
  id: string;
  name: string;
}

/** Where an update stands, as the app reports it. */
export interface UpdateStatus {
  state: 'idle' | 'checking' | 'current' | 'available' | 'installing' | 'ready' | 'blocked' | 'failed';
  version?: string;
  detail?: string;
  stage?: string;
  percent?: number;
}

export interface EnvState {
  accessibility: boolean;
  login: 'enabled' | 'disabled' | 'needs_approval' | 'unavailable';
  /** The language the app itself runs in; the page may show another one until the app is restarted. */
  language: string;
  reduce_transparency: boolean;
  reduce_motion: boolean;
  increase_contrast: boolean;
  /** The window is on the system's glass (Liquid Glass or vibrancy). */
  glass?: boolean;
}

/** The whole state of the settings. Its sections are typed where the shell reads them and loosely where only one page does. */
export interface AppState {
  env: EnvState;
  app: { version: string; bundled: boolean; config_dir: string; usage?: Record<string, number> };
  general: { language: string; show_on_launch: boolean; launch_effect: boolean; menu_icon: boolean; theme: 'auto' | 'light' | 'dark' };
  [section: string]: any;
}

/** What the app sends. */
export type Inbound =
  | { t: 'state'; state: AppState }
  | { t: 'fault'; code: string; detail?: string }
  | { t: 'running'; purpose: Purpose; apps: RunningApp[] }
  /** A shortcut the system would have kept from the page, e.g. Control+Space, said here. */
  | { t: 'chord'; value: string }
  | { t: 'icons'; icons: Record<string, string> }
  | ({ t: 'update' } & UpdateStatus)
  | { t: 'go'; page: string }
  | { t: 'palette_result'; ok: boolean; text: string };

export type InboundHandler = (message: Inbound) => void;
