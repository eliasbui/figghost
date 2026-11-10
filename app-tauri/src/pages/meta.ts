// What the shell knows about the pages without loading any of them: the order of the sidebar, the tabs of the notch, the
// symbol of each page. The sidebar and the tabs are the order the settings have always had.

/** The sidebar: its groups in order, under the heading of each (none for the first). */
export const NAV: Array<{ sec: string; pages: string[] }> = [
  { sec: 'overview', pages: ['general', 'keys'] },
  { sec: 'notch', pages: ['notch', 'islands', 'notifications', 'volume', 'musicwidget'] },
  { sec: 'mouse', pages: ['scroll', 'cursor', 'wheel', 'tiling', 'pin'] },
  { sec: 'text', pages: ['clipboard', 'typing', 'snippets', 'dictation', 'jarvis', 'smart', 'translate', 'rewrite', 'palette'] },
  { sec: 'capture', pages: ['capture', 'vision', 'mirror', 'download', 'browser'] },
  { sec: 'mac', pages: ['utilities', 'network', 'breaks', 'menubar', 'mole'] },
];

/** The tabs of the notch have pages of their own, opened from the list of tabs on the Notch page or from General. */
export const TAB_PAGES: string[] = [
  'agents', 'prs', 'calendar', 'outlook', 'pomodoro', 'dayprogress', 'screentime', 'notes', 'launcher', 'weather',
  'video', 'system', 'updates', 'devices', 'displays', 'present', 'commands', 'shelf', 'media',
];

/** Every page in the order the search lists them: the sidebar's, the tabs', then About. */
export const PAGE_ORDER: string[] = [...NAV.flatMap((g) => g.pages), ...TAB_PAGES, 'about'];

const KNOWN = new Set(PAGE_ORDER);

export function isPage(id: string): boolean {
  return KNOWN.has(id);
}

/** The page the sidebar marks while `id` is open: itself, or the Notch page for the page of one of its tabs. */
export function navPageOf(id: string): string {
  return TAB_PAGES.includes(id) ? 'notch' : id;
}

/** The heading a page is found under, for the search. */
export function secOf(id: string): string {
  const group = NAV.find((g) => g.pages.includes(id));
  return group ? group.sec : TAB_PAGES.includes(id) ? 'tabs' : 'overview';
}

/** The picture of each page: its name in components/macos/Icon.tsx (`pg.<page>`), all of one set and none used twice. */
export const PAGE_SYMBOLS: Record<string, string> = {
  about: 'pg.about',
  agents: 'pg.agents',
  breaks: 'pg.breaks',
  browser: 'pg.browser',
  calendar: 'pg.calendar',
  capture: 'pg.capture',
  clipboard: 'pg.clipboard',
  commands: 'pg.commands',
  cursor: 'pg.cursor',
  dayprogress: 'pg.dayprogress',
  devices: 'pg.devices',
  dictation: 'pg.dictation',
  displays: 'pg.displays',
  download: 'pg.download',
  general: 'pg.general',
  islands: 'pg.islands',
  jarvis: 'pg.jarvis',
  keys: 'pg.keys',
  launcher: 'pg.launcher',
  media: 'pg.media',
  menubar: 'pg.menubar',
  mirror: 'pg.mirror',
  mole: 'pg.mole',
  musicwidget: 'pg.musicwidget',
  network: 'pg.network',
  notch: 'pg.notch',
  notes: 'pg.notes',
  notifications: 'pg.notifications',
  outlook: 'pg.outlook',
  palette: 'pg.palette',
  pin: 'pg.pin',
  pomodoro: 'pg.pomodoro',
  present: 'pg.present',
  prs: 'pg.prs',
  rewrite: 'pg.rewrite',
  screentime: 'pg.screentime',
  scroll: 'pg.scroll',
  shelf: 'pg.shelf',
  smart: 'pg.smart',
  snippets: 'pg.snippets',
  sound: 'pg.sound',
  system: 'pg.system',
  tiling: 'pg.tiling',
  translate: 'pg.translate',
  typing: 'pg.typing',
  updates: 'pg.updates',
  utilities: 'pg.utilities',
  video: 'pg.video',
  vision: 'pg.vision',
  volume: 'pg.volume',
  weather: 'pg.weather',
  wheel: 'pg.wheel',
};

/** The setting that turns a page's feature on and off, or null for a page that is not a feature. */
export function masterPath(id: string, state: Record<string, any>): string | null {
  if (id === 'scroll') return 'mouse.smooth';
  if (id === 'volume') return 'notch.volume_hud';
  const section = state[id];
  return section && typeof section.enabled === 'boolean' ? `${id}.enabled` : null;
}
