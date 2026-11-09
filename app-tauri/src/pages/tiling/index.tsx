import { Button, IconButton } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { cx } from '@/lib/text';
import { shortcutOf } from '@/lib/shortcuts';
import { send, useApp } from '@/stores/app';
import { useSetting, useT } from '@/stores/hooks';
import { SnapCards } from './Snap';

// Where each layout puts a window, as fractions of the screen: left, top, width, height.
const LAYOUT_RECTS: Record<string, number[]> = {
  left: [0, 0, 0.5, 1], right: [0.5, 0, 0.5, 1], top: [0, 0, 1, 0.5], bottom: [0, 0.5, 1, 0.5],
  top_left: [0, 0, 0.5, 0.5], top_right: [0.5, 0, 0.5, 0.5], bottom_left: [0, 0.5, 0.5, 0.5], bottom_right: [0.5, 0.5, 0.5, 0.5],
  maximize: [0, 0, 1, 1], center: [0.2, 0.18, 0.6, 0.64], first_third: [0, 0, 1 / 3, 1], center_third: [1 / 3, 0, 1 / 3, 1], last_third: [2 / 3, 0, 1 / 3, 1],
  grid_1: [0, 0, 1 / 3, 1 / 3], grid_2: [1 / 3, 0, 1 / 3, 1 / 3], grid_3: [2 / 3, 0, 1 / 3, 1 / 3],
  grid_4: [0, 1 / 3, 1 / 3, 1 / 3], grid_5: [1 / 3, 1 / 3, 1 / 3, 1 / 3], grid_6: [2 / 3, 1 / 3, 1 / 3, 1 / 3],
  grid_7: [0, 2 / 3, 1 / 3, 1 / 3], grid_8: [1 / 3, 2 / 3, 1 / 3, 1 / 3], grid_9: [2 / 3, 2 / 3, 1 / 3, 1 / 3],
};

/** One layout: a small picture of where the window goes, its name, its shortcut and a button that clears it. */
function LayoutTile({ layoutKey }: { layoutKey: string }) {
  const t = useT();
  const target = `tiling:${layoutKey}`;
  const set = useApp((s) => !!shortcutOf(s.state, target));
  const [x, y, w, h] = LAYOUT_RECTS[layoutKey] ?? [0, 0, 1, 1];
  return (
    <div className="tile">
      <div className="tile-top">
        <div className="mini">
          <i style={{ left: `${x * 100}%`, top: `${y * 100}%`, width: `${w * 100}%`, height: `${h * 100}%` }} />
        </div>
        <div className="tile-name">{t(`layout.${layoutKey}`)}</div>
      </div>
      <div className="tile-actions">
        <HotkeyButton target={target} allowClear label={t(`layout.${layoutKey}`)} />
        <IconButton icon="close" label={t('tiling.clear')} style={{ visibility: set ? 'visible' : 'hidden' }} onClick={() => send({ t: 'hotkey', target, value: '' })} />
      </div>
    </div>
  );
}

export default function Tiling() {
  const t = useT();
  const enabled = useSetting<boolean>('tiling.enabled');
  const layouts = useSetting<Array<{ key: string }>>('tiling.layouts');
  return (
    <>
      <section className={cx('card', !enabled && 'dim')}>
        <div className="tiles">
          {layouts.map((layout) => (
            <LayoutTile key={layout.key} layoutKey={layout.key} />
          ))}
        </div>
      </section>
      <div>
        <Button small icon="refresh" onClick={() => send({ t: 'action', name: 'reset_tiling' })}>
          {t('tiling.reset')}
        </Button>
      </div>
      <SnapCards />
    </>
  );
}
