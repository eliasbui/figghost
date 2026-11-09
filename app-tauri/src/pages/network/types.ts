// What the state of Network holds, as the page and its diagram read it.

export interface Interface {
  name: string;
}

export interface Wifi {
  grade: string;
  standard?: string;
  channel: number;
  band: string;
  width: number;
  rssi: number;
  noise: number;
  rate: number;
}

export interface SpeedTest {
  state: 'running' | 'done' | 'failed';
  down?: number;
  up?: number | null;
  rtt?: number;
  plain?: boolean;
  reason?: string;
  web?: string;
  https?: string;
}

export interface Link {
  name: string;
  label: string;
  ip?: string;
  quality?: string;
  rtt?: number;
  loss?: number;
  down?: number;
  primary?: boolean;
  wifi?: Wifi | null;
  web?: { http: string; https: string } | null;
  speed_test?: SpeedTest | null;
}

export interface Host {
  name: string;
  received?: number;
  sent?: number;
}

export interface App {
  name: string;
  bundle?: string;
  down: number;
  up: number;
  received: number;
  sent: number;
  interfaces?: Interface[];
  hosts?: Host[];
  held_to?: number;
  refused?: number;
}

export interface Wire {
  app: string;
  target: string;
  priority: string;
  down_kbps?: number;
  up_kbps?: number;
  quota_mb?: number;
  quota_period: string;
}

export interface Metered {
  enabled: boolean;
  allow?: string[];
  cap_mb: number;
  period: string;
  marked?: string[];
  unmarked?: string[];
}

export interface Policy {
  metered_now?: boolean;
  link?: string;
  cap_used?: number;
  cap_level?: string;
  quotas?: Array<{ app: string; used: number; quota: number }>;
}

export interface Tether {
  gnirehtet?: boolean;
  adb?: boolean;
  running?: boolean;
  unauthorized?: boolean;
  phones?: Array<{ model?: string; serial: string }>;
  meter?: { down?: number; up?: number; received?: number; sent?: number };
  test?: { state: string; verdict?: string; phone?: string; ms?: number | null; loss?: number };
}

export interface HistoryRow {
  app: string;
  down: number;
  up: number;
  interfaces?: Interface[];
}

export interface HistoryView {
  today?: HistoryRow[];
  week?: HistoryRow[];
  month?: HistoryRow[];
  daily?: Array<{ day: string; down: number; up: number }>;
  empty?: boolean;
}

export interface NetworkState {
  enabled: boolean;
  control?: boolean;
  running?: boolean;
  paused?: boolean;
  status?: string;
  saved?: boolean;
  total_down: number;
  total_up: number;
  apps?: App[];
  known_apps?: Array<{ name: string; bundle?: string }>;
  links?: Link[];
  known_links?: Array<{ name: string; label: string }>;
  dns: string;
  dns_nextdns: string;
  dns_custom: string;
  metered: Metered;
  policy?: Policy;
  anomaly: boolean;
  anomaly_ignore?: string[];
  history: boolean;
  history_seconds: number;
  history_view?: HistoryView;
  tether?: Tether;
  merge?: string[];
  wires?: Wire[];
  layout?: Record<string, number[]>;
}
