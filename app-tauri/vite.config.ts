import { fileURLToPath, URL } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import type { Plugin } from 'vite';

// The pages of the AppKit app are strings (crates/macos-sys/src/webview.rs loads each with loadHTMLString and nothing may be
// fetched), so the embed build puts the script, the styles, the fonts and the pictures of one surface into one HTML file.
// tools/build-embed.mjs runs it once for each surface and names the surface in SURFACE and the content security policy in
// SURFACE_CSP. `<!--__PRELUDE__-->` is where the app writes what the page needs before it runs (`window.__canvas`, `window.__icon`).
const POLICY = "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:; font-src data:";

/** Text that goes inside a script element must not be able to close it or open a comment. */
const inScript = (code: string) => code.replace(/<\/(script)/gi, '<\\/$1').replace(/<!--/g, '<\\!--');

function oneFile(policy: string): Plugin {
  return {
    name: 'figghost-one-file',
    apply: 'build',
    enforce: 'post',
    generateBundle(_options, bundle) {
      const page = bundle['index.html'];
      if (!page || page.type !== 'asset') throw new Error('one-file: the build has no index.html');
      let html = String(page.source);
      for (const [name, file] of Object.entries(bundle)) {
        if (name === 'index.html') continue;
        if (file.type === 'chunk') {
          const tag = new RegExp(`<script[^>]*src="[^"]*${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}"[^>]*></script>`);
          if (!tag.test(html)) throw new Error(`one-file: no script tag for ${name}`);
          // With the chunks gone there is nothing to preload, and Vite leaves its placeholder for the list of what to preload.
          const code = file.code.replace(/__VITE_PRELOAD__/g, 'void 0');
          html = html.replace(tag, () => `<script type="module">${inScript(code)}</script>`);
        } else if (name.endsWith('.css')) {
          const tag = new RegExp(`<link[^>]*href="[^"]*${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}"[^>]*>`);
          if (!tag.test(html)) throw new Error(`one-file: no link tag for ${name}`);
          html = html.replace(tag, () => `<style>${String(file.source).replace(/<\/(style)/gi, '<\\/$1')}</style>`);
        } else {
          throw new Error(`one-file: ${name} was not inlined`);
        }
        delete bundle[name];
      }
      html = html
        .replace('<head>', `<head>\n    <meta http-equiv="Content-Security-Policy" content="${policy}" />`)
        .replace('<div id="root"></div>', '<div id="root"></div>\n    <!--__PRELUDE__-->');
      page.source = html;
    },
  };
}

/** The one index.html starts `src/main.tsx` (the settings); a surface build starts its own entry instead. */
function surfaceEntry(entry: string): Plugin {
  return {
    name: 'figghost-surface-entry',
    transformIndexHtml: { order: 'pre', handler: (html) => html.replace('/src/main.tsx', entry) },
  };
}

// Tauri loads the dev server at devUrl (src-tauri/tauri.conf.json), so the port is fixed.
export default defineConfig(({ mode }) => {
  const embed = mode === 'embed';
  const entry = process.env.SURFACE_ENTRY;
  return {
    plugins: [react(), tailwindcss(), ...(embed ? [oneFile(process.env.SURFACE_CSP ?? POLICY)] : []), ...(entry ? [surfaceEntry(entry)] : [])],
    resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
    clearScreen: false,
    server: {
      port: 1420,
      strictPort: true,
      watch: { ignored: ['**/src-tauri/**'] },
    },
    build: embed
      ? {
          // tools/build-embed.mjs gives each surface its own folder and copies the page from it to where the app includes it.
          cssCodeSplit: false,
          assetsInlineLimit: Number.MAX_SAFE_INTEGER,
          modulePreload: false,
          chunkSizeWarningLimit: 8000,
          rolldownOptions: { output: { codeSplitting: false } },
        }
      : undefined,
  };
});
