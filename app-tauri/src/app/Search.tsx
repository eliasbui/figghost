import { useEffect, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { IndexingContext } from '@/components/macos/Dialogs';
import { Icon } from '@/components/macos/Icon';
import { PageIcon } from '@/components/macos/Page';
import { translate } from '@/i18n';
import type { Lang } from '@/i18n';
import { folded } from '@/lib/text';
import { PAGE_ORDER, secOf } from '@/pages/meta';
import { pageLoader } from '@/pages/registry';
import { go, useApp } from '@/stores/app';
import { LangOverride, useT } from '@/stores/hooks';

// What each page says, folded: its name, the line under its title, and the titles and notes of its rows. Every page is
// drawn once to a string, without a window and so without any of its effects, the first time a search needs it.
const indexes = new Map<Lang, Promise<Map<string, string>>>();

async function readPages(lang: Lang): Promise<Map<string, string>> {
  const { renderToStaticMarkup } = await import('react-dom/server');
  const texts = new Map<string, string>();
  for (const id of PAGE_ORDER) {
    const parts = [translate(lang, `nav.${id}`), translate(lang, `page.${id}.sub`)];
    try {
      const { default: Page } = await pageLoader(id)();
      const html = renderToStaticMarkup(
        <LangOverride.Provider value={lang}>
          <IndexingContext.Provider value>
            <Page />
          </IndexingContext.Provider>
        </LangOverride.Provider>,
      );
      const doc = new DOMParser().parseFromString(html, 'text/html');
      for (const el of doc.querySelectorAll('.row-title, .row-desc, .row-info, .row-sub, .card-note, .section-title, .feat-name, label, button')) parts.push(el.textContent ?? '');
      for (const el of doc.querySelectorAll('button[aria-label]')) parts.push(el.getAttribute('aria-label') ?? '');
    } catch (error) {
      console.warn(`search: the page ${id} could not be read`, error);
    }
    texts.set(id, folded(parts.join('\n')));
  }
  return texts;
}

function pageTexts(lang: Lang): Promise<Map<string, string>> {
  let index = indexes.get(lang);
  if (!index) {
    index = readPages(lang);
    indexes.set(lang, index);
  }
  return index;
}

/** On the page just opened from a search, brings the first row that says the words into view and marks it. The page
 *  may still be on its way, so it is looked for a few times. */
function reveal(words: string[], tries = 12): void {
  const rows = [...document.querySelectorAll('#scroller .row, #scroller .section-title')];
  const hit = rows.find((row) => words.every((w) => folded(row.textContent ?? '').includes(w)));
  if (!hit) {
    if (tries > 0) setTimeout(() => reveal(words, tries - 1), 100);
    return;
  }
  const still = document.documentElement.classList.contains('reduce-motion');
  hit.scrollIntoView({ block: 'center', behavior: still ? 'auto' : 'smooth' });
  hit.classList.add('found');
  setTimeout(() => hit.classList.remove('found'), 1600);
}

/** The search of the toolbar: the pages that say every word typed, a page whose name says them first. */
export function Search() {
  const t = useT();
  const lang = useApp((s) => s.lang);
  const input = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState('');
  const [focused, setFocused] = useState(false);
  const [selected, setSelected] = useState(0);
  const [index, setIndex] = useState<{ lang: Lang; texts: Map<string, string> } | null>(null);

  const words = folded(query).split(/\s+/).filter(Boolean);

  useEffect(() => {
    if (!words.length) return;
    let live = true;
    pageTexts(lang).then((texts) => {
      if (live) setIndex({ lang, texts });
    });
    return () => {
      live = false;
    };
    // The words are read from the query; the effect only needs to know that there are some.
  }, [lang, words.length > 0]);

  const rank = (id: string) => {
    const name = folded(t(`nav.${id}`));
    return words.every((w) => name.includes(w)) ? (name.startsWith(words[0]) ? 0 : 1) : 2;
  };
  const texts = index && index.lang === lang ? index.texts : null;
  const hits = texts && words.length ? PAGE_ORDER.filter((id) => words.every((w) => (texts.get(id) ?? '').includes(w))).sort((a, b) => rank(a) - rank(b)) : [];
  const open = focused && words.length > 0 && texts !== null;

  // Command-K finds, and Command-F does the same, as in any Mac window.
  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.metaKey && !e.shiftKey && !e.altKey && !e.ctrlKey && (e.code === 'KeyF' || e.code === 'KeyK')) {
        e.preventDefault();
        input.current?.focus();
        input.current?.select();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, []);

  const openResult = (id: string) => {
    go(id);
    reveal(words);
    setQuery('');
    input.current?.blur();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      if (query) setQuery('');
      else input.current?.blur();
    } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && hits.length) {
      e.preventDefault();
      setSelected((now) => (now + (e.key === 'ArrowDown' ? 1 : hits.length - 1)) % hits.length);
    } else if (e.key === 'Enter' && hits.length) {
      openResult(hits[Math.min(selected, hits.length - 1)]);
    }
  };

  return (
    <div className="search">
      <label className="top-search glass">
        <Icon name="search" />
        <input
          ref={input}
          type="search"
          id="search"
          autoComplete="off"
          spellCheck={false}
          placeholder={t('side.search')}
          aria-label={t('side.search')}
          aria-controls="search-results"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setSelected(0);
          }}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          onKeyDown={onKeyDown}
        />
        <kbd aria-hidden="true">{'\u2318'}K</kbd>
      </label>
      <div id="search-results" className="glass-pop" role="listbox" hidden={!open}>
        {hits.length ? (
          hits.map((id, i) => (
            <button
              key={id}
              type="button"
              className="result"
              role="option"
              data-page={id}
              aria-selected={i === selected}
              // The field keeps the focus, so the list stays open until the choice is made.
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => openResult(id)}
            >
              <PageIcon id={id} weight="fill" />
              <span className="result-text">
                <span className="result-name">{t(`nav.${id}`)}</span>
                <span className="result-cat">{t(`nav.sec.${secOf(id)}`)}</span>
              </span>
              <kbd className="result-key" aria-hidden="true">
                {'\u21a9'}
              </kbd>
            </button>
          ))
        ) : (
          <div className="search-empty">{t('side.none')}</div>
        )}
      </div>
    </div>
  );
}
