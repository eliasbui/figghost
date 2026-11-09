import type { Item } from './types';

export function size(n: number): string {
  if (!(n > 0)) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.min(units.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
  return `${(n / 1024 ** i).toFixed(i > 1 ? 1 : 0)} ${units[i]}`;
}

export const sum = (list: Item[]) => list.reduce((n, i) => n + i.size, 0);

export const rate = (n: number) => `${size(n)}/s`;

/** The path with the home folder written as a tilde. */
export const tilde = (path: string, home: string) => (home && path.startsWith(home) ? `~${path.slice(home.length)}` : path);

/** How long, in the words of a person: minutes under an hour, then hours, then days. */
export function agoMinutes(m: number, lang: string): string {
  const vi = lang === 'vi';
  if (m < 60) return `${m} ${vi ? 'phút' : 'min'}`;
  if (m < 1440) return `${Math.floor(m / 60)} ${vi ? 'giờ' : 'h'}`;
  return `${Math.floor(m / 1440)} ${vi ? 'ngày' : 'd'}`;
}
