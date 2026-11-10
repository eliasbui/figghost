// The popup only ever shows a version that is waiting for a decision: available, being installed, installed (a restart
// away), or found but not installable here.

export type State = 'available' | 'installing' | 'ready' | 'blocked';
/** What the app is told when a button is pressed. */
export type Action = 'later' | 'install' | 'restart' | 'download';
export type Label = 'later' | 'close' | 'update' | 'restart' | 'download';

export interface Show {
  t: 'show';
  state: string;
  lang: string;
  version: string;
  current: string;
  changes?: string[];
  stage?: string;
  percent?: number;
  detail?: string;
}

/** The two buttons of each state, the quiet one first; null leaves one out. */
export const BUTTONS: Record<State, readonly [{ label: Label; action: Action }, { label: Label; action: Action } | null]> = {
  available: [{ label: 'later', action: 'later' }, { label: 'update', action: 'install' }],
  installing: [{ label: 'close', action: 'later' }, null],
  ready: [{ label: 'later', action: 'later' }, { label: 'restart', action: 'restart' }],
  blocked: [{ label: 'close', action: 'later' }, { label: 'download', action: 'download' }],
};

export function isState(state: string): state is State {
  return Object.hasOwn(BUTTONS, state);
}

/** The line under the changes: how far an install is, or why one cannot be done here; nothing otherwise. */
export function status(show: Show): { key: 'downloading' | 'installingNow' | 'blockedHint'; vars: Record<string, string | number> } | null {
  if (show.state === 'installing') return show.stage === 'downloading' ? { key: 'downloading', vars: { p: show.percent ?? 0 } } : { key: 'installingNow', vars: {} };
  if (show.state === 'blocked') return { key: 'blockedHint', vars: { d: show.detail ?? '' } };
  return null;
}
