// Shortcuts are kept as text ("cmd+shift+c"); these turn a key press into that text and the text into what is drawn.

import { setIn } from './path';

const MOD_ORDER = ['ctrl', 'opt', 'shift', 'cmd'];
const MOD_ALIASES: Record<string, string> = { command: 'cmd', control: 'ctrl', option: 'opt', alt: 'opt' };
const KEY_ALIASES: Record<string, string> = { enter: 'return', escape: 'esc' };
// The glyphs macOS uses for keys, written as escapes: control, option, shift, command, the four arrows, return, tab,
// escape and delete.
const SYMBOLS: Record<string, string> = {
  ctrl: '\u2303', opt: '\u2325', shift: '\u21e7', cmd: '\u2318', left: '\u2190', right: '\u2192', up: '\u2191', down: '\u2193',
  return: '\u21a9', space: 'Space', tab: '\u21e5', esc: '\u238b', delete: '\u232b',
};
const MODIFIER_CODES = new Set(['ShiftLeft', 'ShiftRight', 'ControlLeft', 'ControlRight', 'AltLeft', 'AltRight', 'MetaLeft', 'MetaRight', 'CapsLock', 'Fn']);
const NAMED_KEYS: Record<string, string> = {
  ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down', Enter: 'return', NumpadEnter: 'return', Space: 'space', Tab: 'tab', Backspace: 'delete',
};

export function isModifierCode(code: string): boolean {
  return MODIFIER_CODES.has(code);
}

/** The key name the app understands for a KeyboardEvent.code, or null when it does not support it. */
export function keyName(code: string): string | null {
  let m: RegExpExecArray | null;
  if ((m = /^Key([A-Z])$/.exec(code))) return m[1].toLowerCase();
  if ((m = /^Digit(\d)$/.exec(code))) return m[1];
  if ((m = /^F(\d{1,2})$/.exec(code)) && +m[1] >= 1 && +m[1] <= 20) return 'f' + m[1];
  return NAMED_KEYS[code] ?? null;
}

/** "cmd+shift+c" as the parts to draw: the modifiers in the order macOS writes them, then the key. */
export function shortcutParts(shortcut: string | null | undefined): string[] {
  const parts = String(shortcut || '')
    .split('+')
    .map((p) => p.trim().toLowerCase())
    .filter(Boolean)
    .map((p) => MOD_ALIASES[p] || KEY_ALIASES[p] || p);
  return [...MOD_ORDER.filter((m) => parts.includes(m)), ...parts.filter((p) => !MOD_ORDER.includes(p))];
}

export function keyLabel(part: string): string {
  return SYMBOLS[part] || part.toUpperCase();
}

/** A shortcut as macOS writes it in a sentence: the glyphs of option, shift and return for "opt+shift+return". */
export function keysText(shortcut: string | null | undefined): string {
  return shortcutParts(shortcut).map(keyLabel).join('');
}

/** The shortcut a key press makes, or why it makes none. */
export function shortcutOfEvent(e: KeyboardEvent): { value: string } | { fault: 'unsupported_key' | 'needs_modifier' } {
  const key = keyName(e.code);
  if (!key) return { fault: 'unsupported_key' };
  const mods: string[] = [];
  if (e.ctrlKey) mods.push('ctrl');
  if (e.altKey) mods.push('opt');
  if (e.shiftKey) mods.push('shift');
  if (e.metaKey) mods.push('cmd');
  if (!mods.length) return { fault: 'needs_modifier' };
  return { value: [...mods, key].join('+') };
}

// -- where the shortcut of a target is kept in the state

interface TilingLayout {
  key: string;
  shortcut: string;
}

/** The shortcut of a target. "clipboard" is clipboard.hotkey; "tiling:left" is the layout `left` of the tiling list;
 *  "<feature>" is <feature>.shortcut and "<feature>:<what>" is <feature>.shortcuts.<what> (so "keys:pin" is
 *  keys.shortcuts.pin). A feature that has no `shortcuts` has one shortcut, in `shortcut`, whatever its target is called:
 *  "breaks:now" is breaks.shortcut. (The page this comes from read breaks.shortcuts.now and so always showed it empty.) */
export function shortcutOf(state: Record<string, any>, target: string): string {
  if (target === 'clipboard') return state.clipboard?.hotkey ?? '';
  const [head, what] = target.split(':');
  const own = state[head];
  if (own && head !== 'tiling') return (what === undefined || !own.shortcuts ? own.shortcut : own.shortcuts[what]) || '';
  const layouts: TilingLayout[] = state.tiling?.layouts ?? [];
  return layouts.find((l) => l.key === what)?.shortcut || '';
}

/** The same target with a new shortcut, for a copy of the state. */
export function withShortcut<T extends Record<string, any>>(state: T, target: string, value: string): T {
  if (target === 'clipboard') return setIn(state, 'clipboard.hotkey', value);
  const [head, what] = target.split(':');
  if (head === 'tiling') {
    const layouts: TilingLayout[] = state.tiling?.layouts ?? [];
    return setIn(
      state,
      'tiling.layouts',
      layouts.map((l) => (l.key === what ? { ...l, shortcut: value } : l)),
    );
  }
  const own = (state as Record<string, any>)[head];
  return setIn(state, what === undefined || !own?.shortcuts ? `${head}.shortcut` : `${head}.shortcuts.${what}`, value);
}
