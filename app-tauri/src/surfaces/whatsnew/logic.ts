// What is new in a version: a list for a few changes, slides for many. A change is a line, "Title: detail".

/** More than this many changes are slides, one each, after a first slide that names the version. */
export const SLIDE_FROM = 4;
/** Dots for at most this many slides: more would not fit between the two buttons, so a count takes their place. */
export const MAX_DOTS = 9;

export interface Show {
  t: 'show';
  lang: string;
  version: string;
  changes?: string[];
}

/** "Title: detail" gives a slide a heading and a line under it; a line without that stays a heading. */
export function split(line: string): { title: string; detail: string } {
  const at = line.indexOf(': ');
  if (at > 0 && at <= 48 && line.length > at + 2) {
    const detail = line.slice(at + 2);
    return { title: line.slice(0, at), detail: detail[0].toUpperCase() + detail.slice(1) };
  }
  return { title: line, detail: '' };
}

export const isSlides = (changes: readonly string[]): boolean => changes.length >= SLIDE_FROM;

/** The slide before the changes counts too. */
export const slideCount = (changes: readonly string[]): number => changes.length + 1;

export const countsInstead = (count: number): boolean => count > MAX_DOTS;

/** What the button on the right of the slides says. */
export function nextLabel(index: number, count: number): 'start' | 'next' | 'done' {
  if (index >= count - 1) return 'done';
  return index === 0 ? 'start' : 'next';
}

export const clamp = (index: number, count: number): number => Math.max(0, Math.min(count - 1, index));
