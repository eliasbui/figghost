import { SelectRow, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { hasKey } from '@/i18n';
import { useLang, useSection, useT } from '@/stores/hooks';
import { PermissionRow } from '@/components/macos/Common';

// The keys, in the order of the page: the one that starts first, then the ones that work while presenting.
const ACTIONS = ['start', 'zoom_in', 'zoom_out', 'zoom_reset', 'auto', 'spotlight', 'ring', 'freeze', 'black', 'white'];

/** Present: a tab of the notch that shows a display, an area, a window or an app to an audience on another display or in a
 *  window of its own, with a zoom that follows the pointer, a spotlight, a ring around the pointer, a held picture, a black
 *  or white screen, and keys for all of it. The presenting itself runs in the app. */
export default function Present() {
  const t = useT();
  const lang = useLang();
  const p = useSection<{ enabled: boolean; screen_access: boolean }>('present');
  const on = p.enabled;
  const pick = (path: string, key: string, options: Array<{ value: number | string; label: string }>) => (
    <SelectRow path={`present.${path}`} title={t(`present.${key}.title`)} desc={t(`present.${key}.desc`)} when={on} options={options} />
  );
  return (
    <>
      <Card>
        <PermissionRow title={t('present.access.title')} given={p.screen_access} detail={p.screen_access ? undefined : t('present.access.no')} action="present_screen_access" label={t('present.access.ask')} />
      </Card>

      <SectionTitle>{t('sec.present.what')}</SectionTitle>
      <Card>
        {pick('source', 'source', ['display', 'area', 'window', 'app'].map((v) => ({ value: v, label: t(`present.source.${v}`) })))}
        {pick('fps', 'fps', [30, 60].map((n) => ({ value: n, label: String(n) })))}
        <ToggleRow path="present.cursor" title={t('present.cursor.title')} desc={t('present.cursor.desc')} when={on} />
        <ToggleRow path="present.border" title={t('present.border.title')} desc={t('present.border.desc')} when={on} />
      </Card>

      <SectionTitle>{t('sec.present.zoom')}</SectionTitle>
      <Card>
        <ToggleRow path="present.auto_zoom" title={t('present.auto.title')} desc={t('present.auto.desc')} when={on} />
        {pick('zoom', 'zoom', [1.5, 2, 2.5, 3, 4].map((n) => ({ value: n, label: `${String(n).replace('.', lang === 'vi' ? ',' : '.')}x` })))}
        {pick('zoom_out', 'zoomout', [0, 2, 3, 5, 8].map((n) => ({ value: n, label: n ? t('present.seconds', { n }) : t('present.zoomout.never') })))}
      </Card>

      <SectionTitle>{t('sec.present.pointer')}</SectionTitle>
      <Card>
        <ToggleRow path="present.ring" title={t('present.ring.title')} desc={t('present.ring.desc')} when={on} />
        <ToggleRow path="present.clicks" title={t('present.clicks.title')} desc={t('present.clicks.desc')} when={on} />
        {pick('spotlight', 'spot', [[12, 'small'], [18, 'medium'], [26, 'large']].map(([n, k]) => ({ value: n, label: t(`present.spot.${k}`) })))}
      </Card>

      <SectionTitle>{t('sec.present.notch')}</SectionTitle>
      <Card>
        <ToggleRow path="present.closed_notch" title={t('present.closed.title')} desc={t('present.closed.desc')} when={on} />
      </Card>

      <SectionTitle info={t('present.keys.desc')}>{t('sec.present.keys')}</SectionTitle>
      <Card>
        {ACTIONS.map((action) => (
          <Row key={action} title={t(`present.key.${action}`)} desc={hasKey(`present.key.${action}.desc`) ? t(`present.key.${action}.desc`) : null} when={on}>
            <HotkeyButton target={`present:${action}`} allowClear label={t(`present.key.${action}`)} />
          </Row>
        ))}
      </Card>
    </>
  );
}
