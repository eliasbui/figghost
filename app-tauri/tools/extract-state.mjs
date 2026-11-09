// Builds the sample state of the settings page: web/fixture.json with the part of each feature (web/features/<id>.fixture.json)
// under its id, the way fixture() in crates/app-ui/src/settings/page.rs does it.
//
//   node tools/extract-state.mjs ../crates/app-ui src/data/state.sample.json
//
// These are the values the old page is previewed with, not the defaults of the real configuration: a count or a list of
// apps in them is a sample. The Tauri side starts from them until the model of the settings is moved out of app-ui.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const [appUi, output] = process.argv.slice(2);
if (!appUi || !output) {
  console.error('usage: node tools/extract-state.mjs <crates/app-ui> <output file>');
  process.exit(2);
}
const web = join(appUi, 'web');
const pageRs = readFileSync(join(appUi, 'src/settings/page.rs'), 'utf8');
const start = pageRs.indexOf('const FEATURE_FIXTURES');
const block = pageRs.slice(start, pageRs.indexOf('];', start));
const parts = [...block.matchAll(/\(\s*"([a-z_]+)"\s*,\s*include_str!\("\.\.\/\.\.\/web\/features\/([a-z_]+\.fixture\.json)"\)\s*,?\s*\)/g)];
if (parts.length < 30) throw new Error(`only ${parts.length} fixtures found in settings/page.rs; its layout has changed`);

const state = JSON.parse(readFileSync(join(web, 'fixture.json'), 'utf8'));
for (const [, id, file] of parts) state[id] = JSON.parse(readFileSync(join(web, 'features', file), 'utf8'));

mkdirSync(dirname(output), { recursive: true });
writeFileSync(output, `${JSON.stringify(state, null, 1)}\n`);
console.log(`${Object.keys(state).length} sections, ${parts.length} of them from features`);
