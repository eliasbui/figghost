export type Lang = 'en' | 'vi';
export type Words = Record<string, string>;

/** `key` in the language, else in English, else the key itself; `{name}` in the words is filled from `vars`. */
export function say(words: Record<Lang, Words>, lang: string, key: string, vars?: Record<string, string | number | null | undefined>): string {
  const text = (lang === 'vi' ? words.vi[key] : undefined) ?? words.en[key] ?? key;
  return text.replace(/\{(\w+)\}/g, (_, name: string) => (vars && vars[name] != null ? String(vars[name]) : ''));
}
