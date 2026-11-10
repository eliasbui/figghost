// The music widget: what the app says about the song, and what is drawn from it.

export type Style = 'card' | 'cover' | 'bar' | 'vinyl' | 'minimal';
export const STYLES: readonly Style[] = ['card', 'cover', 'bar', 'vinyl', 'minimal'];

export interface Track {
  title?: string;
  artist?: string;
  album?: string;
  app?: string;
  playing?: boolean;
  /** Where the song is when the app looked, and how long it is, in milliseconds. */
  pos: number;
  dur?: number;
}

export interface State {
  t: 'state';
  style: string;
  ambient?: boolean;
  radius?: number;
  labels?: Record<string, string>;
  track: Track | null;
  /** Sent with the first message and whenever the picture changes: a data URL, or null for none. */
  art?: string | null;
}

export interface View {
  style: Style;
  ambient: boolean;
  radius: number;
  labels: Record<string, string>;
  track: Track | null;
  art: string | null;
  /** When the message came, so that the bar moves between messages. */
  at: number;
}

export const FIRST_VIEW: View = { style: 'card', ambient: true, radius: 26, labels: {}, track: null, art: null, at: 0 };

export function isStyle(style: string): style is Style {
  return (STYLES as readonly string[]).includes(style);
}

/** The view after a message. The picture is kept until the app sends another, and there is none without a song. */
export function next(view: View, message: State, now: number): View {
  const art = 'art' in message ? (message.art ?? null) : view.art;
  return {
    style: isStyle(message.style) ? message.style : 'card',
    ambient: !!message.ambient,
    radius: message.radius ?? view.radius,
    labels: message.labels ?? view.labels,
    track: message.track,
    art: message.track ? art : null,
    at: now,
  };
}

/** Where the song is now: what the app said, and the time since if it plays. */
export function position(track: Track, at: number, now: number): number {
  const shown = track.pos + (track.playing ? now - at : 0);
  return track.dur ? Math.min(shown, track.dur) : shown;
}

export function clock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

/** Where a press on the bar lands in the song, in milliseconds. */
export function seekTo(track: Track, fraction: number): number {
  return Math.round((track.dur ?? 0) * Math.min(1, Math.max(0, fraction)));
}
