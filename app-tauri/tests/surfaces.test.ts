// What the surfaces of the AppKit app and the beta decide for themselves, apart from how they look. The logic files import
// nothing, so Node runs them as they are: `pnpm test`.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { say } from '../src/surfaces/text.ts';
import { arrange } from '../src/surfaces/tray/logic.ts';
import type { Item } from '../src/surfaces/tray/logic.ts';
import { BUTTONS, isState, status } from '../src/surfaces/update/logic.ts';
import { MAX_DOTS, clamp, countsInstead, isSlides, nextLabel, slideCount, split } from '../src/surfaces/whatsnew/logic.ts';
import { clock, next, position, seekTo } from '../src/surfaces/widget/logic.ts';
import type { State, View } from '../src/surfaces/widget/logic.ts';
import { ago, characters, newNoteId, noteTitle } from '../src/surfaces/desktop/logic.ts';

let id = 0;
const item = (title: string, symbol: string, checked: boolean | null = null, visible = true): Item => ({ id: String(id++), title, symbol, icon: null, checked, visible });

test('the menu is laid out by what each entry is, not by where it is in the list', () => {
  const entries = [
    item('Settings', 'gearshape'),
    item('Accessibility is off', 'hand.raised'),
    item('Smooth scrolling', 'computermouse', true),
    item('Keep on top', 'pin', false),
    item('Capture Area', 'rectangle.dashed'),
    item('Stop Recording', 'stop.circle'),
    item('Stop Recording', 'stop.circle'),
    item('Hidden tool', 'eyedropper', null, false),
    item('Open clipboard history', 'doc.on.clipboard'),
    item('Clear history', 'trash'),
    item('Start at login', 'power', true),
    item('Open settings folder', 'folder'),
    item('Check for Updates...', 'arrow.triangle.2.circlepath'),
    item('Quit', 'xmark.rectangle'),
  ];
  const a = arrange(entries);
  assert.equal(a.settings?.title, 'Settings');
  assert.equal(a.access?.title, 'Accessibility is off');
  assert.deepEqual(a.toggles.map((i) => i.title), ['Smooth scrolling', 'Keep on top']);
  assert.deepEqual(a.tools.map((i) => i.title), ['Capture Area', 'Stop Recording'], 'a tool that is in the menu twice is one tile, and a hidden one is not there');
  assert.deepEqual(a.clip.map((i) => i.title), ['Open clipboard history', 'Clear history']);
  assert.equal(a.login?.title, 'Start at login');
  assert.equal(a.config?.title, 'Open settings folder');
  assert.equal(a.updates?.title, 'Check for Updates...');
  assert.equal(a.quit?.title, 'Quit');
});

test('quit is the last entry of its kind and takes nothing else with it', () => {
  const a = arrange([item('Close', 'xmark.rectangle'), item('Quit', 'xmark.rectangle')]);
  assert.equal(a.quit?.title, 'Quit');
  assert.deepEqual(a.tools.map((i) => i.title), ['Close']);
});

test('the update popup has the buttons and the line its state calls for', () => {
  assert.ok(isState('available') && isState('blocked') && !isState('idle') && !isState('toString'));
  assert.deepEqual(BUTTONS.available.map((b) => b?.action), ['later', 'install']);
  assert.deepEqual(BUTTONS.ready.map((b) => b?.action), ['later', 'restart']);
  assert.deepEqual(BUTTONS.blocked.map((b) => b?.action), ['later', 'download']);
  assert.equal(BUTTONS.installing[1], null, 'nothing to press while it installs');
  const base = { t: 'show', lang: 'en', version: '9.9.9', current: '1.0.0' } as const;
  assert.deepEqual(status({ ...base, state: 'installing', stage: 'downloading', percent: 42 }), { key: 'downloading', vars: { p: 42 } });
  assert.equal(status({ ...base, state: 'installing', stage: 'checking' })?.key, 'installingNow');
  assert.deepEqual(status({ ...base, state: 'blocked', detail: 'no permission' }), { key: 'blockedHint', vars: { d: 'no permission' } });
  assert.equal(status({ ...base, state: 'available' }), null);
  assert.equal(status({ ...base, state: 'ready' }), null);
});

test('a change is a title and a detail, and few changes are a list while many are slides', () => {
  assert.deepEqual(split('Add a thing: it does this'), { title: 'Add a thing', detail: 'It does this' });
  assert.deepEqual(split('Fix another'), { title: 'Fix another', detail: '' });
  assert.deepEqual(split('Ends with a colon: '), { title: 'Ends with a colon: ', detail: '' });
  const long = `${'x'.repeat(49)}: detail`;
  assert.deepEqual(split(long), { title: long, detail: '' }, 'a title that long is a sentence, not a title');
  assert.equal(isSlides(['a', 'b', 'c']), false);
  assert.equal(isSlides(['a', 'b', 'c', 'd']), true);
  assert.equal(slideCount(['a', 'b', 'c', 'd']), 5, 'the slide that names the version counts');
});

test('the slides count their way to the last one', () => {
  assert.equal(nextLabel(0, 6), 'start');
  assert.equal(nextLabel(3, 6), 'next');
  assert.equal(nextLabel(5, 6), 'done');
  assert.equal(clamp(-1, 6), 0);
  assert.equal(clamp(9, 6), 5);
  assert.equal(countsInstead(MAX_DOTS), false);
  assert.equal(countsInstead(22), true, 'twenty-one changes are a count, not dots');
});

const view = (over: Partial<View> = {}): View => ({ style: 'card', ambient: true, radius: 26, labels: {}, track: null, art: null, at: 0, ...over });
const song = { title: 'Midnight City', playing: true, pos: 96000, dur: 243000 };
const state = (over: Partial<State> = {}): State => ({ t: 'state', style: 'vinyl', ambient: true, track: song, ...over });

test('the widget keeps the picture until the app sends another, and has none without a song', () => {
  const first = next(view(), state({ art: 'data:a' }), 1000);
  assert.equal(first.art, 'data:a');
  assert.equal(first.style, 'vinyl');
  assert.equal(first.at, 1000);
  assert.equal(next(first, state(), 2000).art, 'data:a', 'a message without `art` leaves it');
  assert.equal(next(first, state({ art: null }), 2000).art, null, 'null takes it away');
  assert.equal(next(first, state({ track: null }), 2000).art, null, 'no song, no picture');
  assert.equal(next(view(), state({ style: 'nonsense' }), 0).style, 'card', 'an unknown style is the card');
  assert.equal(next(view({ radius: 13 }), state(), 0).radius, 13, 'the radius stays when the app does not say');
});

test('the bar moves while the song plays, stops at its end, and a press on it lands in the song', () => {
  assert.equal(position(song, 1000, 3000), 98000);
  assert.equal(position({ ...song, playing: false }, 1000, 3000), 96000);
  assert.equal(position(song, 0, 10_000_000), 243000);
  assert.equal(position({ pos: 5000 }, 0, 0), 5000, 'a song of unknown length has no end');
  assert.equal(clock(96000), '1:36');
  assert.equal(clock(243000), '4:03');
  assert.equal(clock(-5), '0:00');
  assert.equal(seekTo(song, 0.5), 121500);
  assert.equal(seekTo(song, 2), 243000);
  assert.equal(seekTo(song, -1), 0);
});

test('words come in the language, else in English, else as the key', () => {
  const words = { en: { hello: 'Hello {name}', only: 'Only English' }, vi: { hello: 'Xin chào {name}' } };
  assert.equal(say(words, 'vi', 'hello', { name: 'An' }), 'Xin chào An');
  assert.equal(say(words, 'vi', 'only'), 'Only English');
  assert.equal(say(words, 'en', 'missing'), 'missing');
  assert.equal(say(words, 'en', 'hello'), 'Hello ', 'a name nobody gave is left out');
});

test('the beta says how long ago, titles a note, and numbers a new one', () => {
  const now = 1_000_000_000;
  assert.deepEqual(ago(now - 10_000, now), { key: 'ago.now', n: 0 });
  assert.deepEqual(ago(now - 120_000, now), { key: 'ago.m', n: 2 });
  assert.deepEqual(ago(now - 3 * 3600_000, now), { key: 'ago.h', n: 3 });
  assert.deepEqual(ago(now - 2 * 86400_000, now), { key: 'ago.d', n: 2 });
  assert.equal(ago(now + 5000, now).key, 'ago.now', 'a clock that is ahead is just now');
  assert.equal(noteTitle('\n  ## Shopping\nmilk', 'New note'), 'Shopping');
  assert.equal(noteTitle('   \n ', 'New note'), 'New note');
  assert.equal(noteTitle('a'.repeat(100), 'x').length, 60);
  assert.equal(characters('a\u{1D11E}b'), 3);
  assert.equal(newNoteId([{ id: now + 50, text: '', updated: 0 }], now), now + 51);
  assert.equal(newNoteId([], now), now);
});
