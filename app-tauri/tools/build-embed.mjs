// Builds the pages that the apps of this repository carry inside them: one HTML file for each surface, with its script,
// styles, fonts and pictures inline, written beside the Rust code that includes it.
//
//   node tools/build-embed.mjs            every surface
//   node tools/build-embed.mjs tray       only the ones named
//
// The files it writes are committed, so that building the apps needs no Node. They are build output: change the source
// under src/ and run this again.
import { copyFileSync, mkdirSync, readFileSync, rmSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { build } from 'vite';

const APP_UI = '../crates/app-ui/web';
const SURFACES = {
  settings: { entry: '/src/main.tsx', out: `${APP_UI}/settings.html` },
  tray: { entry: '/src/surfaces/tray/index.tsx', out: `${APP_UI}/tray.html` },
  permissions: { entry: '/src/surfaces/permissions/index.tsx', out: `${APP_UI}/permissions.html` },
  update: { entry: '/src/surfaces/update/index.tsx', out: `${APP_UI}/update.html` },
  whatsnew: { entry: '/src/surfaces/whatsnew/index.tsx', out: `${APP_UI}/whatsnew.html` },
  outlook: { entry: '/src/surfaces/outlook/index.tsx', out: `${APP_UI}/outlook.html` },
  widget: { entry: '/src/surfaces/widget/index.tsx', out: `${APP_UI}/widget.html` },
  // The beta for Windows and Linux serves its page and its pictures from a custom protocol of its own.
  desktop: {
    entry: '/src/surfaces/desktop/index.tsx',
    out: '../crates/desktop/web/index.html',
    policy: "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src 'self' data:; font-src data:",
  },
};

const wanted = process.argv.slice(2);
const names = wanted.length ? wanted : Object.keys(SURFACES);
for (const name of names) if (!SURFACES[name]) throw new Error(`no surface is named ${name}; they are ${Object.keys(SURFACES).join(', ')}`);

for (const name of names) {
  const { entry, out, policy } = SURFACES[name];
  const dir = join('node_modules', '.embed', name);
  if (name === 'settings') delete process.env.SURFACE_ENTRY;
  else process.env.SURFACE_ENTRY = entry;
  if (policy) process.env.SURFACE_CSP = policy;
  else delete process.env.SURFACE_CSP;
  rmSync(dir, { recursive: true, force: true });
  await build({ mode: 'embed', logLevel: 'warn', build: { outDir: dir, emptyOutDir: true } });

  const page = join(dir, 'index.html');
  const html = readFileSync(page, 'utf8');
  // Nothing may be fetched when the page opens: the elements of the page (what is left when the insides of its script and
  // style elements are taken out) name no address and no file, and the styles load nothing.
  const bare = html.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, '<script></script>').replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, '<style></style>');
  const styles = [...html.matchAll(/<style\b[^>]*>([\s\S]*?)<\/style>/gi)].map((m) => m[1]).join('\n');
  for (const [where, text, fetching] of [
    ['an element', bare, [/<link\b/i, /\bsrc="https?:/i, /\bhref="https?:/i]],
    ['a style', styles, [/url\(\s*["']?https?:/i, /@import/i]],
  ]) {
    for (const bad of fetching) if (bad.test(text)) throw new Error(`${name}: ${where} fetches something (${bad})`);
  }
  mkdirSync(dirname(out), { recursive: true });
  copyFileSync(page, out);
  console.log(`${name.padEnd(12)} ${String(Math.round(statSync(out).size / 1024)).padStart(5)} KB  ${out}`);
}
