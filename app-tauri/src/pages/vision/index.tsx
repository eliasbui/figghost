import { IconButton, Segmented, ActionIcon } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { change, send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './vision.css';

const FORMATS = ['hex', 'rgb', 'hsl', 'hsb'];

interface VisionState {
  enabled: boolean;
  shortcut: string;
  format: string;
  screen_access: boolean;
  background_removal: boolean;
}

/** Colour picker, and background removal for images. */
export default function Vision() {
  const t = useT();
  const vision = useSection<VisionState>('vision');
  const on = vision.enabled;
  return (
    <>
      <SectionTitle>{t('sec.vision.keys')}</SectionTitle>
      <Card>
        <Row title={t('vision.shortcut.title')} desc={t('vision.shortcut.desc')} when={on}>
          <div className="vision-controls">
            <HotkeyButton target="vision" allowClear label={t('vision.shortcut.title')} />
            <IconButton
              icon="close"
              label={t('vision.shortcut.clear')}
              style={{ visibility: vision.shortcut ? 'visible' : 'hidden' }}
              onClick={() => send({ t: 'hotkey', target: 'vision', value: '' })}
            />
          </div>
        </Row>
        <Row title={t('vision.format.title')} desc={t('vision.format.desc')} when={on}>
          <Segmented label={t('vision.format.title')} value={vision.format} options={FORMATS.map((f) => ({ value: f, label: f.toUpperCase() }))} onPick={(f) => change('vision.format', f)} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.vision.access')}</SectionTitle>
      <Card>
        <Row title={t('vision.access.title')} desc={t('vision.access.desc')}>
          <Pill good={vision.screen_access}>{t(vision.screen_access ? 'vision.access.granted' : 'vision.access.missing')}</Pill>
          <ActionIcon name="vision_screen_access" icon="open" label={t('vision.access.allow')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.vision.more')}</SectionTitle>
      <Card>
        <Row title={t('vision.bg.title')} desc={t('vision.bg.desc')}>
          <Pill good={vision.background_removal}>{t(vision.background_removal ? 'vision.bg.yes' : 'vision.bg.no')}</Pill>
        </Row>
      </Card>
    </>
  );
}
