// What the beta of Figghost for Windows and Linux knows, and the few rules the window follows. The words are in strings.ts.

export interface Env {
  os: string;
  version: string;
  lang: string;
  wayland: boolean;
  hotkey_error: string | null;
  /** Null when the system cannot start an app at login. */
  autostart: boolean | null;
  exe: string;
  data_dir: string;
  count: number;
  agents_error: string | null;
}

export interface Config {
  language: string;
  theme: string;
  hotkey: string;
  history_limit: number;
  secrets: string;
  mcp: { enabled: boolean };
  agents: { enabled: boolean; approval_wait_seconds: number };
}

export interface Approval {
  id: number;
  agent: string;
  tool: string;
  detail: string;
  question: string;
  project: string;
  requested: number;
}

export interface Session {
  agent: string;
  project: string;
  note: string;
  updated: number;
  state: string;
}

export interface Agents {
  agents: { id: string; name: string; installed: boolean }[];
  approvals: Approval[];
  sessions: Session[];
}

export interface State {
  t: 'state';
  env: Env;
  config: Config;
  agents: Agents;
}

export interface Item {
  id: number;
  kind: string;
  preview: string;
  image: string | null;
  width: number;
  height: number;
  used: number;
  pinned: boolean;
  masked: boolean;
}

export interface Note {
  id: number;
  text: string;
  updated: number;
}

export type Incoming =
  | State
  | { t: 'items'; q: string; items: Item[] }
  | { t: 'changed' }
  | { t: 'notes'; notes: Note[] }
  | { t: 'agents'; agents: Agents }
  | { t: 'go'; page: string }
  | { t: 'focus_search' }
  | { t: 'error'; message: string };

export const PAGES = ['clipboard', 'notes', 'agents', 'settings', 'about'] as const;
export type Page = (typeof PAGES)[number];
export const isPage = (id: string): id is Page => (PAGES as readonly string[]).includes(id);

export const OS_NAMES: Record<string, string> = { windows: 'Windows', linux: 'Linux', macos: 'macOS' };

/** How long ago a time was, as the key of the words and the number to put in them. */
export function ago(then: number, now: number): { key: 'ago.now' | 'ago.m' | 'ago.h' | 'ago.d'; n: number } {
  const seconds = Math.max(0, (now - then) / 1000);
  if (seconds < 45) return { key: 'ago.now', n: 0 };
  if (seconds < 3600) return { key: 'ago.m', n: Math.max(1, Math.round(seconds / 60)) };
  if (seconds < 86400) return { key: 'ago.h', n: Math.round(seconds / 3600) };
  return { key: 'ago.d', n: Math.round(seconds / 86400) };
}

/** The first line of a note that has words, without its hashes, as its title. */
export function noteTitle(text: string, untitled: string): string {
  const line = text
    .split('\n')
    .map((l) => l.trim())
    .find((l) => l);
  return line ? line.replace(/^#+\s*/, '').slice(0, 60) : untitled;
}

/** The characters of a text as a person counts them (an emoji is one). */
export const characters = (text: string): number => [...text].length;

/** The number a new note gets: the time, or one more than the highest if that is already taken. */
export function newNoteId(notes: readonly Note[], now: number): number {
  return Math.max(now, ...notes.map((n) => n.id + 1));
}
