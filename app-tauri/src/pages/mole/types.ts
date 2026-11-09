// What the state of Mole holds, as the page reads it. The app fills it in as each scan finishes; a list that has not been
// scanned yet is null.

export interface Item {
  path: string;
  name: string;
  note: string;
  size: number;
  dir?: boolean;
  /** Days since a folder was touched (project build folders). */
  age?: number | null;
  /** How many leftovers an app has besides itself. */
  extra?: number;
  state?: string;
  status?: string;
  logs?: number;
  used?: number;
  anonymous?: boolean;
  kind?: string;
  ports?: number[];
  minutes?: number;
}

export interface Process {
  name: string;
  pid: number;
  cpu: number;
  memory: number;
}

export interface Status {
  at: number | null;
  score: number | null;
  cpu: number;
  memory: number;
  gpu: number | null;
  disk: number;
  net_down: number;
  net_up: number;
  cores?: number[];
  kinds?: [number, number];
  memory_total?: number;
  memory_used?: number;
  memory_app?: number;
  memory_wired?: number;
  memory_compressed?: number;
  pressure?: string;
  swap_used?: number;
  swap_total?: number;
  disk_total?: number;
  disk_used?: number;
  celsius?: number | null;
  thermal?: string;
  fans?: Array<{ rpm: number; max?: number }>;
  battery?: { health: number; cycles: number };
  top_cpu?: Process[];
  top_memory?: Process[];
}

export interface Analysis {
  path: string;
  total: number | null;
  entries: Item[];
}

export interface Docker {
  ok: boolean;
  missing?: boolean;
  down?: boolean;
  disk?: number;
  logs: number;
  totals?: Array<{ type: string; size: number; reclaimable: number }>;
  containers: Item[];
  images: Item[];
  volumes: Item[];
  networks: Item[];
}

export interface MoleResult {
  seq: number;
  removed: number;
  failed: number;
  freed: number;
  procs?: boolean;
  docker?: boolean;
}

export interface MoleState {
  home: string;
  busy: string[];
  status: Status | null;
  clean: Item[] | null;
  purge: Item[] | null;
  installers: Item[] | null;
  apps: Item[] | null;
  analyze: Analysis | null;
  docker: Docker | null;
  procs: { ok: boolean; items: Item[] } | null;
  result: MoleResult | null;
}
