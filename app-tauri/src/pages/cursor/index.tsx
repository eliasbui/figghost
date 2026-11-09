import { SegRow, SliderRow, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { Icon } from '@/components/macos/Icon';
import { Card, InfoRow, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { change, send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './cursor.css';

interface CursorState {
  enabled: boolean;
  theme: string;
  themes: string[];
  sheets?: Record<string, string>;
  custom_name: string;
  unreadable?: boolean;
}

/** The picture on a tile: the rainbow arrow is drawn, a character is a sheet of frames, your own picture is a folder to open. */
function Art({ theme, sheets }: { theme: string; sheets?: Record<string, string> }) {
  if (theme === 'neon') {
    return (
      <div className="cursor-art">
        <div className="cursor-neon-frame">
          <div className="cursor-neon" />
        </div>
      </div>
    );
  }
  if (theme === 'custom') {
    return (
      <div className="cursor-art">
        <Icon name="folder" />
      </div>
    );
  }
  return (
    <div className="cursor-art">
      <div className="cursor-sprite" role="img" style={{ backgroundImage: `url("${sheets?.[theme] ?? ''}")` }} />
    </div>
  );
}

/** Custom cursor: the arrow and the hand over links become a rainbow arrow, a pixel character or a picture of your own. */
export default function Cursor() {
  const t = useT();
  const cursor = useSection<CursorState>('cursor');
  const on = cursor.enabled;
  const choose = () => send({ t: 'action', name: 'cursor_choose' });
  return (
    <>
      <SectionTitle>{t('sec.cursor.look')}</SectionTitle>
      <Card>
        <Row title={t('cursor.theme.title')} desc={t('cursor.theme.desc')} block when={on}>
          <div className="cursor-tiles" role="radiogroup" aria-label={t('cursor.theme.title')}>
            {cursor.themes.map((theme) => (
              <button
                key={theme}
                type="button"
                className="cursor-tile"
                role="radio"
                aria-checked={cursor.theme === theme}
                data-theme={theme}
                // Until a picture has been chosen, the tile of your own picture asks for one.
                onClick={() => (theme === 'custom' && !cursor.custom_name ? choose() : change('cursor.theme', theme))}
              >
                <Art theme={theme} sheets={cursor.sheets} />
                <span className="cursor-label">{t(`cursor.theme.${theme}`)}</span>
              </button>
            ))}
          </div>
        </Row>
        <Row title={t('cursor.custom.title')} desc={t('cursor.custom.desc')} when={on}>
          {cursor.unreadable ? <Pill>{t('cursor.custom.bad')}</Pill> : null}
          <span className="cursor-file">{cursor.custom_name || t('cursor.custom.none')}</span>
          <ActionIcon name="cursor_choose" icon="folder" label={t('cursor.custom.choose')} />
        </Row>
        <SegRow path="cursor.tip" title={t('cursor.tip.title')} desc={t('cursor.tip.desc')} values={['top', 'centre']} when={on && cursor.theme === 'custom'} />
      </Card>

      <SectionTitle>{t('sec.cursor.feel')}</SectionTitle>
      <Card>
        <SliderRow path="cursor.size" title={t('cursor.size.title')} desc={t('cursor.size.desc')} min={16} max={96} step={2} format={(v) => `${Math.round(v)} pt`} />
        <SliderRow path="cursor.speed" title={t('cursor.speed.title')} desc={t('cursor.speed.desc')} min={0.25} max={3} step={0.05} format={(v) => `${v.toFixed(2).replace(/0$/, '')}x`} />
      </Card>
      <Card>
        <InfoRow title={t('cursor.note.title')} desc={t('cursor.note.desc')} />
      </Card>
    </>
  );
}
