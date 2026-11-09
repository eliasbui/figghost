import { Button } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { cx } from '@/lib/text';
import { change, send } from '@/stores/app';
import { useSetting, useT } from '@/stores/hooks';

// The look and the place of the volume indicator: a card for each style and a small screen to pick the place on.
const STYLES = ['notch', 'glass', 'corner', 'bar', 'island', 'side', 'tile', 'ring', 'equalizer', 'wide'];
const PLACES = ['top-left', 'top', 'top-right', 'left', 'center', 'right', 'bottom-left', 'bottom', 'bottom-right'];
// The styles that hang from the notch stay under it, so a place means nothing for them.
const HANGING = ['notch', 'glass', 'bar'];

function StyleCards() {
  const t = useT();
  const on = useSetting<boolean>('notch.volume_hud');
  const current = useSetting<string>('notch.hud');
  return (
    <>
      <SectionTitle>{t('vol.style.title')}</SectionTitle>
      <div className={cx('style-wrap', !on && 'disabled')}>
        <div className="style-cards hud-cards" role="group" aria-label={t('vol.style.title')}>
          {STYLES.map((id) => (
            <button key={id} type="button" className="style-card" aria-pressed={current === id} onClick={() => change('notch.hud', id)}>
              <span className={`sc-art hud-art hud-${id}`} aria-hidden="true">
                <i />
              </span>
              <span className="sc-name">{t(`vol.style.${id}`)}</span>
              <span className="sc-desc">{t(`vol.style.${id}.desc`)}</span>
            </button>
          ))}
        </div>
      </div>
    </>
  );
}

function PlaceRows() {
  const t = useT();
  const on = useSetting<boolean>('notch.volume_hud');
  const style = useSetting<string>('notch.hud');
  const place = useSetting<string>('notch.hud_position');
  return (
    <Card>
      <Row title={t('vol.place.title')} desc={t('vol.place.desc')} when={on && !HANGING.includes(style)}>
        <div className="hud-place-box">
          <div className="hud-places" role="group" aria-label={t('vol.place.title')}>
            {PLACES.map((p) => (
              <button key={p} type="button" className="hud-place" aria-pressed={place === p} aria-label={t(`vol.place.${p}`)} title={t(`vol.place.${p}`)} onClick={() => change('notch.hud_position', p)} />
            ))}
          </div>
          <Button small className={cx(place === 'auto' && 'on')} onClick={() => change('notch.hud_position', 'auto')}>
            {t('vol.place.auto')}
          </Button>
        </div>
      </Row>
      <Row title={t('vol.preview.title')} desc={t('vol.preview.desc')} when={on}>
        <Button small onClick={() => send({ t: 'action', name: 'hud_preview' })}>
          {t('vol.preview')}
        </Button>
      </Row>
    </Card>
  );
}

/** The volume indicator in the notch: its switch (at the top of the page), its look and where a floating one stands. */
export default function Volume() {
  return (
    <>
      <StyleCards />
      <PlaceRows />
    </>
  );
}
