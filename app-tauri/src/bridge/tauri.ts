import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Bridge } from './index';
import type { Inbound } from './messages';

// The command and the event of src-tauri/src/lib.rs.
const COMMAND = 'fig_send';
const EVENT = 'fig:recv';

export function tauriBridge(): Bridge {
  return {
    kind: 'tauri',
    send(message) {
      invoke(COMMAND, { message }).catch((error) => console.error(`${COMMAND}: ${String(error)}`));
    },
    async subscribe(handler) {
      return listen<Inbound>(EVENT, (event) => handler(event.payload));
    },
  };
}
