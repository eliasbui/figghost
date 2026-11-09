import { ActionButton, ActionIcon } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { useSection, useT } from '@/stores/hooks';
import './pin.css';

interface PinState {
  enabled: boolean;
  pinned: string;
  accessibility: boolean;
  screen_access: boolean;
}

/** Always on top: a shortcut keeps the window in front above the others, with a red border round it. */
export default function Pin() {
  const t = useT();
  const pin = useSection<PinState>('pin');
  const on = pin.enabled;
  return (
    <>
      <Card>
        <Row title={t('pin.shortcut.title')} desc={t('pin.shortcut.desc')} when={on}>
          <HotkeyButton target="pin" allowClear label={t('pin.shortcut.title')} />
        </Row>
        <Row title={t('pin.now.title')} when={on}>
          <span className="row-title pin-now" title={pin.pinned || ''}>
            {pin.pinned || t('pin.now.none')}
          </span>
          <ActionButton name="pin_release" style={{ visibility: pin.pinned ? 'visible' : 'hidden' }}>
            {t('pin.now.release')}
          </ActionButton>
        </Row>
      </Card>

      <SectionTitle>{t('sec.pin.access')}</SectionTitle>
      <Card>
        <Row title={t('pin.access.title')} desc={t('pin.access.desc')}>
          <Pill good={pin.accessibility}>{t(pin.accessibility ? 'pin.allowed' : 'pin.not_allowed')}</Pill>
          <ActionIcon name="open_accessibility" icon="open" label={t('pin.access.open')} />
        </Row>
        <Row title={t('pin.screen.title')} desc={t('pin.screen.desc')}>
          <Pill good={pin.screen_access}>{t(pin.screen_access ? 'pin.allowed' : 'pin.not_allowed')}</Pill>
          <ActionIcon name="pin_screen_access" icon="open" label={t('pin.screen.allow')} />
        </Row>
      </Card>
    </>
  );
}
