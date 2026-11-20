import type { ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { Theme } from '@astryxdesign/core/theme';
import { neutralTheme } from '@astryxdesign/theme-neutral/built';

/** Puts a surface on the page, under the theme of Astryx: it follows the system's light and dark. */
export function mount(surface: ReactNode): void {
  createRoot(document.getElementById('root')!).render(<Theme theme={neutralTheme}>{surface}</Theme>);
}
