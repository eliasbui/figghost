import type { Link, NetworkState } from './types';

/** A speed in bytes a second, as the page says it. */
export function speed(b: number | null | undefined): string {
  const n = Number(b) || 0;
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)} GB/s`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)} MB/s`;
  if (n >= 1e3) return `${Math.round(n / 1e3)} KB/s`;
  return `${n} B/s`;
}

/** An amount of data in bytes. */
export function size(b: number | null | undefined): string {
  const n = Number(b) || 0;
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)} GB`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)} MB`;
  if (n >= 1e3) return `${Math.round(n / 1e3)} KB`;
  return `${n} B`;
}

/** The name System Settings gives a connection, for the interface it is on. */
export function linkName(network: NetworkState, iface: string): string {
  const all = [...(network.links ?? []), ...(network.known_links ?? [])];
  return all.find((x) => x.name === iface)?.label ?? iface;
}

/** The glyph of a connection, from what System Settings calls it. */
export function linkSymbol(l: Pick<Link, 'label'>): string {
  const label = String(l.label || '').toLowerCase();
  if (label.includes('wi-fi') || label.includes('wifi') || label.includes('airport')) return 'wifi';
  if (label.includes('iphone') || label.includes('android') || label.includes('phone')) return 'iphone';
  if (/ethernet|lan|usb|thunderbolt|dock|adapter/.test(label)) return 'cable.connector';
  return 'network';
}

/** A speed in bits a second, for the result of a test. */
export function mbps(bps: number | null | undefined): string {
  const n = Number(bps) || 0;
  return n >= 1e9 ? `${(n / 1e9).toFixed(2)} Gbps` : `${(n / 1e6).toFixed(1)} Mbps`;
}
