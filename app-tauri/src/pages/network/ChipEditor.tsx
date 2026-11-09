import { useState } from 'react';
import type { ReactNode } from 'react';
import { IconButton, Select } from '@/components/macos/Controls';
import { send } from '@/stores/app';
import type { NetworkState } from './types';

/** A list of app names kept as chips, with a menu of the apps seen to add one: the allowed apps, the apps left alone.
 *  The chips and the menu stand in different rows, so they come back apart. */
export function useChipEditor({ network, path, list, none, add, remove }: { network: NetworkState; path: string; list: string[]; none: string; add: string; remove: (app: string) => string }): { controls: ReactNode; chips: ReactNode } {
  const [picked, setPicked] = useState('');
  const write = (names: string[]) => send({ t: 'set', path, value: names });
  const seen = [...new Set([...(network.known_apps ?? []).map((a) => a.name), ...(network.apps ?? []).map((a) => a.name)])]
    .filter((n) => !list.some((x) => x.toLowerCase() === n.toLowerCase()))
    .sort((a, b) => a.localeCompare(b));
  const value = seen.includes(picked) ? picked : (seen[0] ?? '');
  return {
    controls: (
      <div className="network-allow">
        <Select label={add} value={value} disabled={!seen.length} options={seen.map((n) => ({ value: n, label: n }))} onChange={(v) => setPicked(String(v))} />
        <IconButton variant="glass" icon="plus" disabled={!seen.length} label={add} onClick={() => value && write([...list, value])} />
      </div>
    ),
    chips: (
      <div className="network-chips">
        {list.length ? (
          list.map((name) => (
            <span className="network-chip" key={name}>
              {name}
              <button type="button" className="network-chip-x" aria-label={remove(name)} onClick={() => write(list.filter((n) => n !== name))}>
                {'\u00d7'}
              </button>
            </span>
          ))
        ) : (
          <span className="row-desc">{none}</span>
        )}
      </div>
    ),
  };
}
