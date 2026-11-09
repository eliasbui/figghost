import { createContext, useCallback, useContext } from 'react';
import { translate } from '@/i18n';
import type { Lang } from '@/i18n';
import { getIn } from '@/lib/path';
import { useApp } from './app';

export type TFunction = (key: string, vars?: Record<string, string | number>) => string;

/** The value of a setting by its path ("mouse.step_px"); the page draws again only when that value changes. */
export function useSetting<T = any>(path: string): T {
  return useApp((s) => getIn(s.state, path) as T);
}

/** A whole section of the state ("pomodoro"). */
export function useSection<T = any>(name: string): T {
  return useApp((s) => s.state[name] as T);
}

/** A language other than the one of the page, for the search, which reads the words of every page in the language of the field. */
export const LangOverride = createContext<Lang | null>(null);

/** The words of the page in its language; the page draws again when the language changes. */
export function useT(): TFunction {
  const forced = useContext(LangOverride);
  const stored = useApp((s) => s.lang);
  const lang = forced ?? stored;
  return useCallback((key, vars) => translate(lang, key, vars), [lang]);
}

/** The language the page is in. */
export function useLang(): Lang {
  const forced = useContext(LangOverride);
  const stored = useApp((s) => s.lang);
  return forced ?? stored;
}
