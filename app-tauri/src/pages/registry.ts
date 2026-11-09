import type { ComponentType } from 'react';

// A page is the folder pages/<id>/ with its component as the default export of index.tsx. The pages are found here, so
// that one is added by adding its folder; a page that has not been written yet shows a notice in its place.
const modules = import.meta.glob<{ default: ComponentType }>('./*/index.tsx');

/** The pages that open on a top of their own (the app's, with its icon) and not the one of a feature. */
const OWN_TOP = new Set(['general', 'about']);

export function pageLoader(id: string): () => Promise<{ default: ComponentType }> {
  return modules[`./${id}/index.tsx`] ?? (() => import('./Pending'));
}

export function hasOwnTop(id: string): boolean {
  return OWN_TOP.has(id);
}

export function isWritten(id: string): boolean {
  return `./${id}/index.tsx` in modules;
}
