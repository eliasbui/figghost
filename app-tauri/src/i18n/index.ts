// The words of the page. en.json and vi.json are written by tools/extract-i18n.mjs from the dictionaries of the current
// page (app.js and the features), so both languages keep the keys they have there: `t('pomodoro.enable.title')`.
import en from './en.json';
import { shell } from './shell';
import vi from './vi.json';

export type Lang = 'en' | 'vi';

const DICTIONARIES: Record<Lang, Record<string, string>> = { en: { ...en, ...shell.en }, vi: { ...vi, ...shell.vi } };

/** English has every key, so it is the one that says whether a key exists. */
export function hasKey(key: string): boolean {
  return key in DICTIONARIES.en;
}

/** The text of a key in a language, with `{name}` replaced by the value of `name`; English, then the key, when it is missing. */
export function translate(lang: Lang, key: string, vars?: Record<string, string | number>): string {
  let text = DICTIONARIES[lang][key] || DICTIONARIES.en[key] || key;
  if (vars) for (const [name, value] of Object.entries(vars)) text = text.replaceAll('{' + name + '}', String(value));
  return text;
}

/** The language of the page for the setting: a language as it is, or for `auto` the first language of the system that has words here. */
export function resolveLang(setting: string): Lang {
  if (setting === 'en' || setting === 'vi') return setting;
  for (const tag of navigator.languages?.length ? navigator.languages : [navigator.language || 'en']) {
    const primary = String(tag).slice(0, 2).toLowerCase();
    if (primary === 'en' || primary === 'vi') return primary;
  }
  return 'en';
}
