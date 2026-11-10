// The entries of the menu come from the app with their title, symbol, check and visibility; the panel lays them out by
// what they are, not by where they are in the list.

export interface Item {
  id: string;
  title: string;
  symbol: string;
  /** The SF Symbol drawn by the system, as a data URL. */
  icon: string | null;
  /** Null for an entry that is not a switch. */
  checked: boolean | null;
  visible: boolean;
}

export interface Arranged {
  settings?: Item;
  access?: Item;
  login?: Item;
  config?: Item;
  updates?: Item;
  quit?: Item;
  clip: Item[];
  toggles: Item[];
  tools: Item[];
}

const is =
  (...symbols: string[]) =>
  (item: Item) =>
    symbols.includes(item.symbol);

export const isStop = is('stop.circle', 'stop.circle.fill');
export const isTrash = is('trash', 'trash.fill');

export function arrange(items: readonly Item[]): Arranged {
  const shown = items.filter((item) => item.visible);
  const used = new Set<string>();
  const take = (test: (item: Item) => boolean) => {
    const found = shown.filter((item) => !used.has(item.id) && test(item));
    for (const item of found) used.add(item.id);
    return found;
  };
  const settings = take(is('gearshape', 'gearshape.fill'))[0];
  const access = take(is('hand.raised', 'hand.raised.fill'))[0];
  // Quit is the last of its kind, so that a tool with the same symbol earlier in the list stays a tool.
  const quit = [...shown].reverse().find(is('xmark.rectangle', 'xmark.rectangle.fill'));
  if (quit) used.add(quit.id);
  const login = take((item) => item.symbol === 'power' && item.checked !== null)[0];
  const config = take(is('folder', 'folder.fill'))[0];
  const updates = take(is('arrow.triangle.2.circlepath'))[0];
  const clip = take(is('doc.on.clipboard', 'doc.on.clipboard.fill', 'trash', 'trash.fill'));
  const toggles = take((item) => item.checked !== null);
  // A tool that is in the menu twice, as Stop recording is, is one tile.
  const seen = new Set<string>();
  const tools = take(() => true).filter((item) => {
    const key = `${item.symbol}|${item.title}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  return { settings, access, login, config, updates, quit, clip, toggles, tools };
}
