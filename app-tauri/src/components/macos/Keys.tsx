import { keyLabel, shortcutParts } from '@/lib/shortcuts';

/** A shortcut as the keys of a keyboard. */
export function Keycaps({ shortcut }: { shortcut: string }) {
  return (
    <span className="keys">
      {shortcutParts(shortcut).map((part) => (
        <kbd key={part}>{keyLabel(part)}</kbd>
      ))}
    </span>
  );
}

export function CodeBlock({ children, label }: { children: string; label?: string }) {
  return (
    <pre className="code" tabIndex={0} aria-label={label}>
      {children}
    </pre>
  );
}
