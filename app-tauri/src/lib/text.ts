/** Text without case or Vietnamese marks, so "cuon" finds "Cuộn" and "hover" finds "Hover delay". */
export function folded(text: string): string {
  return String(text).normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/[\u0111\u0110]/g, 'd').toLowerCase();
}

/** The first sentence of a text. */
export function firstSentence(text: string): string {
  return String(text).split(/(?<=\.)\s/)[0];
}

/** Class names that are set, joined. */
export function cx(...names: Array<string | false | null | undefined>): string {
  return names.filter(Boolean).join(' ');
}

const locale = (lang: string) => (lang === 'vi' ? 'vi-VN' : 'en-US');

/** 1,240,000 as "1.2M". */
export function compact(n: number, lang: string): string {
  return new Intl.NumberFormat(locale(lang), { notation: 'compact', maximumFractionDigits: n < 1e4 ? 1 : 2 }).format(n);
}

export function usd(amount: number, lang: string): string {
  return new Intl.NumberFormat(locale(lang), { style: 'currency', currency: 'USD', maximumFractionDigits: amount >= 1000 ? 0 : 2 }).format(amount);
}

/** A number of bytes as B, KB, MB or GB. */
export function bytes(n: number): string {
  if (!(n > 0)) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.min(units.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
  return (n / 1024 ** i).toFixed(i > 1 ? 1 : 0) + ' ' + units[i];
}

export function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}
