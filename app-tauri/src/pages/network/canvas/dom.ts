import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { button as buttonClass } from '@/components/macos/Button';
import { Icon } from '@/components/macos/Icon';
import { useApp, wantIcon } from '@/stores/app';

// The diagram is drawn with the DOM directly, as the page it comes from drew everything: it moves hundreds of times a second
// while a node is dragged, and a React render for each of them would only be in the way. These are the few helpers it needs.

type Kid = Node | string | number | null | undefined | false;
type Attrs = Record<string, unknown> | null;

/** An element with attributes and children; `class`, `text`, `html` and `on<event>` are the names it understands. */
export function h<K extends keyof HTMLElementTagNameMap>(tag: K, attrs?: Attrs, ...kids: Array<Kid | Kid[]>): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs ?? {})) {
    if (value === false || value == null) continue;
    if (key === 'class') el.className = String(value);
    else if (key === 'text') el.textContent = String(value);
    else if (key === 'html') el.innerHTML = String(value);
    else if (key.startsWith('on') && typeof value === 'function') el.addEventListener(key.slice(2), value as EventListener);
    else el.setAttribute(key, value === true ? '' : String(value));
  }
  for (const kid of kids.flat()) {
    if (kid == null || kid === false) continue;
    el.append(kid instanceof Node ? kid : document.createTextNode(String(kid)));
  }
  return el;
}

/** A symbol of the page, as an element. */
export function iconEl(name: string, cls?: string): HTMLElement {
  const template = document.createElement('template');
  template.innerHTML = renderToStaticMarkup(createElement(Icon, { name, className: cls }));
  return template.content.firstElementChild as HTMLElement;
}

interface ButtonOptions {
  primary?: boolean;
  danger?: boolean;
  small?: boolean;
  icon?: string;
  aria?: string;
}

export function button(label: string, onclick: (e: MouseEvent) => void, opts: ButtonOptions = {}): HTMLButtonElement {
  const only = !label && !!opts.icon;
  const cls = buttonClass({ variant: opts.primary ? 'prominent' : opts.danger ? 'danger' : 'glass', size: opts.small ? 'sm' : 'md', only: only || undefined });
  return h('button', { class: cls, type: 'button', onclick, 'aria-label': opts.aria, title: only ? opts.aria : undefined }, opts.icon ? iconEl(opts.icon) : null, label);
}

/** The icon of an app: asked of the app for, and drawn once it arrives (see `refreshIcons`). */
export function iconImg(id: string): HTMLImageElement {
  const img = h('img', { class: 'app-icon', alt: '', 'data-id': id });
  const src = useApp.getState().icons[id];
  if (src) img.setAttribute('src', src);
  else wantIcon(id);
  return img;
}

/** Puts the pictures that have come in on the images that wait for them. */
export function refreshIcons(root: ParentNode): void {
  const icons = useApp.getState().icons;
  root.querySelectorAll<HTMLImageElement>('img[data-id]').forEach((img) => {
    const url = icons[img.dataset.id ?? ''];
    if (url && img.getAttribute('src') !== url) img.setAttribute('src', url);
  });
}
