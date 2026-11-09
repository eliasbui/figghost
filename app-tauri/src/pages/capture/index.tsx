import { SliderRow, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, InfoRow, Row, SectionTitle } from '@/components/macos/Layout';
import { useSection, useT } from '@/stores/hooks';
import { PermissionRow } from '@/components/macos/Common';

const KINDS = ['area', 'window', 'screen', 'timer', 'text', 'scroll', 'record', 'gif'];
// Shortcuts that are not a kind of capture.
const ACTIONS = ['again', 'folder'];
const RECORD = ['system_audio', 'microphone', 'camera', 'clicks', 'keys'];

interface CaptureState {
  enabled: boolean;
  screen_access: boolean;
  folder_path: string;
  recording_supported: boolean;
}

/** Screenshots and recordings: what happens after a capture, the shortcuts of each kind, and what a recording includes.
 *  The capture itself runs in the app. */
export default function Capture() {
  const t = useT();
  const capture = useSection<CaptureState>('capture');
  const on = capture.enabled;
  return (
    <>
      <Card>
        <PermissionRow title={t('capture.access.title')} given={capture.screen_access} detail={capture.screen_access ? undefined : t('capture.access.no')} action="capture_screen_access" label={t('capture.access.ask')} />
      </Card>

      <SectionTitle>{t('sec.capture.after')}</SectionTitle>
      <Card>
        <ToggleRow path="capture.save" title={t('capture.save.title')} desc={t('capture.save.desc')} when={on} />
        <Row title={t('capture.folder.title')}>
          <span className="row-value" title={capture.folder_path}>
            {capture.folder_path}
          </span>
          <ActionIcon name="capture_open_folder" icon="folder" label={t('capture.folder.open')} />
        </Row>
        <ToggleRow path="capture.copy" title={t('capture.copy.title')} desc={t('capture.copy.desc')} when={on} />
        <ToggleRow path="capture.quick_panel" title={t('capture.quick.title')} desc={t('capture.quick.desc')} when={on} />
        <SliderRow
          path="capture.quick_seconds"
          title={t('capture.quick_seconds.title')}
          min={0}
          max={30}
          step={1}
          format={(v) => (v === 0 ? t('capture.quick_seconds.forever') : t('capture.seconds', { n: v }))}
        />
        <SliderRow path="capture.timer_seconds" title={t('capture.timer.title')} min={1} max={30} step={1} format={(v) => t('capture.seconds', { n: v })} />
      </Card>

      <SectionTitle>{t('sec.capture.shortcuts')}</SectionTitle>
      <Card>
        {KINDS.concat(ACTIONS).map((kind) => (
          <Row key={kind} title={t(`capture.kind.${kind}`)} when={on}>
            <HotkeyButton target={`capture:${kind}`} allowClear label={t(`capture.kind.${kind}`)} />
          </Row>
        ))}
      </Card>

      <SectionTitle>{t('sec.capture.record')}</SectionTitle>
      <Card>
        {capture.recording_supported ? null : <InfoRow title={t('capture.record.unsupported')} />}
        {RECORD.map((key) => (
          <ToggleRow key={key} path={`capture.record.${key}`} title={t(`capture.record.${key}.title`)} desc={t(`capture.record.${key}.desc`)} when={on && capture.recording_supported} />
        ))}
      </Card>
    </>
  );
}
