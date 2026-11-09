import { ActionIcon, SelectRow, ToggleRow } from '@/components/macos/Controls';
import { PermissionRow } from '@/components/macos/Common';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { useSection, useT } from '@/stores/hooks';

interface MirrorState {
  enabled: boolean;
  camera: boolean;
  has_note: boolean;
}

/** Mirror and teleprompter: the camera under the notch, and the note you wrote last scrolling near the lens. */
export default function Mirror() {
  const t = useT();
  const mirror = useSection<MirrorState>('mirror');
  const on = mirror.enabled;
  return (
    <>
      <SectionTitle>{t('sec.mirror.mirror')}</SectionTitle>
      <Card>
        <Row title={t('mirror.shortcut.title')} desc={t('mirror.shortcut.desc')} when={on}>
          <HotkeyButton target="mirror:mirror" allowClear label={t('mirror.shortcut.title')} />
        </Row>
        <SelectRow path="mirror.mirror_size" title={t('mirror.size.title')} desc={t('mirror.size.desc')} when={on} options={[200, 280, 360].map((n) => ({ value: n, label: t('mirror.size.n', { n }) }))} />
        <PermissionRow title={t('mirror.camera.title')} detail={t('mirror.camera.desc')} given={mirror.camera} action="open_camera_settings" label={t('mirror.camera.open')} />
        <Row title={t('mirror.toggle')} desc={t('mirror.toggle.desc')} when={on}>
          <ActionIcon name="mirror_toggle" icon="play" variant="prominent" label={t('mirror.toggle')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.mirror.prompter')}</SectionTitle>
      <Card>
        <Row title={t('mirror.prompter.shortcut.title')} desc={t('mirror.prompter.shortcut.desc')} when={on}>
          <HotkeyButton target="mirror:prompter" allowClear label={t('mirror.prompter.shortcut.title')} />
        </Row>
        <Row title={t('mirror.note.title')} desc={t('mirror.note.desc')} when={on}>
          <Pill good={mirror.has_note}>{mirror.has_note ? t('ui.ready') : t('mirror.note.none')}</Pill>
        </Row>
        <SelectRow path="mirror.text_size" title={t('mirror.text.title')} desc={t('mirror.text.desc')} when={on} options={[28, 36, 48, 60].map((n) => ({ value: n, label: t('mirror.size.n', { n }) }))} />
        <SelectRow path="mirror.speed" title={t('mirror.speed.title')} desc={t('mirror.speed.desc')} when={on} options={[1, 2, 3, 4, 5, 6].map((n) => ({ value: n, label: String(n) }))} />
        <ToggleRow path="mirror.hide_in_capture" title={t('mirror.hide.title')} desc={t('mirror.hide.desc')} when={on} />
        <Row title={t('mirror.prompter.toggle')} desc={t('mirror.prompter.toggle.desc')} when={on}>
          <ActionIcon name="mirror_prompter" icon="play" variant="prominent" label={t('mirror.prompter.toggle')} />
        </Row>
      </Card>
    </>
  );
}
