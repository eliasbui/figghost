// Reads the settings page of the current app (crates/app-ui/web) and writes its words, in both languages, as JSON.
//
//   node tools/extract-i18n.mjs ../crates/app-ui src/i18n
//
// app.js is only parsed: the object literals of its dictionaries are evaluated on their own, in an empty context, and
// nothing else of it runs. A feature file is run in an empty context with a bare `window`, because the words it adds
// are built inside it; its `build()` is never called. The order is the one the page uses (the base dictionary, then the
// features in the order of FEATURE_SCRIPTS in settings/page.rs), so a later key replaces an earlier one as it does there.
//
// Writes en.json, vi.json (one flat dictionary each) and plugins.json (id, icon and the pages each feature adds
// cards to), and prints the keys that exist in one language only.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import vm from 'node:vm';

const require = createRequire(import.meta.url);
// ACORN_PATH lets the tool run where acorn is not installed beside it.
const acorn = require(process.env.ACORN_PATH || 'acorn');

const [appUi, outDir] = process.argv.slice(2);
if (!appUi || !outDir) {
  console.error('usage: node tools/extract-i18n.mjs <crates/app-ui> <output folder>');
  process.exit(2);
}
const web = join(appUi, 'web');
const pageRs = readFileSync(join(appUi, 'src/settings/page.rs'), 'utf8');

function rustList(name) {
  const start = pageRs.indexOf(`const ${name}:`);
  if (start < 0) throw new Error(`${name} not found in settings/page.rs`);
  return pageRs.slice(start, pageRs.indexOf('];', start));
}
const featureIds = [...rustList('FEATURE_SCRIPTS').matchAll(/web\/features\/([a-z_]+)\.js/g)].map((m) => m[1]);

const en = {};
const vi = {};
const empty = () => Object.create(null);

// -- app.js
const appSource = readFileSync(join(web, 'app.js'), 'utf8');
const ast = acorn.parse(appSource, { ecmaVersion: 'latest', sourceType: 'script' });
const literal = (node) => vm.runInNewContext(`(${appSource.slice(node.start, node.end)})`, empty());

let assigned = 0;
for (const node of ast.body) {
  if (node.type === 'VariableDeclaration') {
    for (const decl of node.declarations) {
      if (decl.id.name !== 'I18N') continue;
      const dictionaries = literal(decl.init);
      Object.assign(en, dictionaries.en);
      Object.assign(vi, dictionaries.vi);
      assigned++;
    }
  } else if (node.type === 'ExpressionStatement' && node.expression.type === 'CallExpression') {
    const call = node.expression;
    const callee = call.callee;
    const isAssign = callee.type === 'MemberExpression' && callee.object.name === 'Object' && callee.property.name === 'assign';
    const [target, source] = call.arguments;
    if (!isAssign || call.arguments.length !== 2) continue;
    if (target.type !== 'MemberExpression' || target.object.name !== 'I18N' || source.type !== 'ObjectExpression') continue;
    Object.assign(target.property.name === 'en' ? en : vi, literal(source));
    assigned++;
  }
}
if (assigned < 5) throw new Error(`only ${assigned} dictionaries found in app.js; its layout has changed`);

// -- the features
const plugins = [];
const replaced = [];
for (const id of featureIds) {
  const file = join(web, 'features', `${id}.js`);
  const sandbox = { window: {}, console };
  vm.runInNewContext(readFileSync(file, 'utf8'), sandbox, { filename: file });
  const added = sandbox.window.__plugins || [];
  if (!added.length) throw new Error(`${file} registers no feature`);
  for (const plugin of added) {
    for (const [target, words] of [[en, plugin.en], [vi, plugin.vi]]) {
      for (const [key, value] of Object.entries(words || {})) {
        if (key in target && target[key] !== value) replaced.push(`${plugin.id}: ${key}`);
        target[key] = value;
      }
    }
    plugins.push({
      id: plugin.id,
      icon: plugin.icon || null,
      slots: plugin.slots ? Object.keys(plugin.slots) : [],
      canvas: typeof plugin.canvas === 'function',
      file: `${id}.js`,
    });
  }
}

mkdirSync(outDir, { recursive: true });
const write = (name, value) => writeFileSync(join(outDir, name), `${JSON.stringify(value, null, 1)}\n`);
write('en.json', en);
write('vi.json', vi);
write('plugins.json', plugins);

const onlyEn = Object.keys(en).filter((k) => !(k in vi));
const onlyVi = Object.keys(vi).filter((k) => !(k in en));
console.log(`en ${Object.keys(en).length} keys, vi ${Object.keys(vi).length} keys, ${plugins.length} features`);
if (replaced.length) console.log(`replaced by a later dictionary (${replaced.length}):\n  ${replaced.join('\n  ')}`);
if (onlyEn.length) console.log(`only in en (${onlyEn.length}):\n  ${onlyEn.join('\n  ')}`);
if (onlyVi.length) console.log(`only in vi (${onlyVi.length}):\n  ${onlyVi.join('\n  ')}`);
