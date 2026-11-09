import { SelectRow, SliderRow, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { cx } from '@/lib/text';
import { change } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './musicwidget.css';

const STYLES = ['card', 'cover', 'bar', 'vinyl', 'minimal'];

interface WidgetState {
  enabled: boolean;
  style: string;
}

/** Music widget: a player in Liquid Glass that floats on the desktop, in a few styles, anywhere on the screen. */
export default function MusicWidget() {
  const t = useT();
  const widget = useSection<WidgetState>('musicwidget');
  const on = widget.enabled;
  const size = (field: 'width' | 'height', min: number, max: number) => (
    <SliderRow
      path={`musicwidget.${field}`}
      title={t(`musicwidget.${field}.title`)}
      desc={t(`musicwidget.${field}.desc`)}
      min={min}
      max={max}
      step={10}
      format={(v) => (v === 0 ? t('musicwidget.auto') : t('musicwidget.px', { n: Math.round(v) }))}
    />
  );
  return (
    <>
      <SectionTitle>{t('sec.musicwidget.style')}</SectionTitle>
      <div className={cx('style-wrap', !on && 'disabled')}>
        <div className="style-cards mw-cards" role="group" aria-label={t('sec.musicwidget.style')}>
          {STYLES.map((id) => (
            <button key={id} type="button" className="style-card" aria-pressed={widget.style === id} onClick={() => change('musicwidget.style', id)}>
              <span className={`sc-art mw-art mw-${id}`} aria-hidden="true">
                <i className="mw-cover" />
                <i className="mw-l1" />
                <i className="mw-l2" />
                <i className="mw-bar" />
              </span>
              <span className="sc-name">{t(`musicwidget.style.${id}`)}</span>
              <span className="sc-desc">{t(`musicwidget.style.${id}.desc`)}</span>
            </button>
          ))}
        </div>
      </div>

      <SectionTitle>{t('sec.musicwidget.glass')}</SectionTitle>
      <Card>
        <SelectRow
          path="musicwidget.material"
          title={t('musicwidget.material.title')}
          desc={t('musicwidget.material.desc')}
          when={on}
          options={['glass', 'clear', 'dark', 'accent'].map((v) => ({ value: v, label: t(`musicwidget.material.${v}`) }))}
        />
        <ToggleRow path="musicwidget.ambient" title={t('musicwidget.ambient.title')} desc={t('musicwidget.ambient.desc')} when={on} />
        <SliderRow path="musicwidget.opacity" title={t('musicwidget.opacity.title')} desc={t('musicwidget.opacity.desc')} min={0.3} max={1} step={0.05} format={(v) => `${Math.round(v * 100)}%`} />
      </Card>

      <SectionTitle>{t('sec.musicwidget.place')}</SectionTitle>
      <Card>
        <SelectRow
          path="musicwidget.layer"
          title={t('musicwidget.layer.title')}
          desc={t('musicwidget.layer.desc')}
          when={on}
          options={['desktop', 'top', 'always', 'lockscreen'].map((v) => ({ value: v, label: t(`musicwidget.layer.${v}`) }))}
        />
        <ToggleRow path="musicwidget.hide_idle" title={t('musicwidget.hide_idle.title')} desc={t('musicwidget.hide_idle.desc')} when={on} />
        {size('width', 180, 700)}
        {size('height', 56, 600)}
        <Row title={t('musicwidget.reset.title')} desc={t('musicwidget.reset.desc')} when={on}>
          <ActionIcon name="musicwidget_reset" icon="refresh" label={t('musicwidget.reset')} />
        </Row>
      </Card>
    </>
  );
}
