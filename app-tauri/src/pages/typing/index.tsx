import { AddAppButton, AppList, AppRow } from '@/components/macos/Apps';
import { ActionButton, IconButton, SegRow, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, InfoRow, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';

interface TypingState {
  enabled: boolean;
  running: boolean;
  vietnamese: boolean;
  accessibility: boolean;
  other?: string;
  apps: Array<{ id: string; name: string }>;
}

/** Vietnamese typing: Telex or VNI in every app, with the engine of Go Nhanh. */
export default function Typing() {
  const t = useT();
  const ty = useSection<TypingState>('typing');
  const on = ty.enabled;
  return (
    <>
      {on && ty.other ? (
        <Card>
          <Row title={t('typing.other.title', { name: ty.other })} desc={t('typing.other.desc', { name: ty.other })} />
        </Card>
      ) : null}
      <Card>
        <Row title={t('typing.now.title')} desc={t('typing.now.desc')} when={on && ty.running}>
          <Pill good={ty.vietnamese}>{t(ty.vietnamese ? 'typing.now.vi' : 'typing.now.en')}</Pill>
          <ActionButton name="typing_toggle">{t('typing.now.switch')}</ActionButton>
        </Row>
        <Row title={t('typing.shortcut')} desc={t('typing.shortcut.desc')} when={on}>
          <HotkeyButton target="typing" allowClear label={t('typing.shortcut')} />
        </Row>
        <ToggleRow path="typing.notch_badge" title={t('typing.notch.title')} desc={t('typing.notch.desc')} when={on} />
        <ToggleRow path="typing.tray_badge" title={t('typing.tray.title')} desc={t('typing.tray.desc')} when={on} />
        <ToggleRow path="typing.flag" title={t('typing.flag.title')} desc={t('typing.flag.desc')} when={on} />
        <ToggleRow path="typing.waving_flag" title={t('typing.waving.title')} desc={t('typing.waving.desc')} when={on} />
        <Row title={t('typing.access.title')} desc={t('typing.access.desc')} when={on}>
          <Pill good={ty.accessibility}>{t(ty.accessibility ? 'access.granted' : 'access.missing')}</Pill>
          <ActionIcon name="open_accessibility" icon="open" label={t('typing.access.open')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.typing.how')}</SectionTitle>
      <Card>
        <SegRow path="typing.method" title={t('typing.method')} desc={t('typing.method.desc')} values={['telex', 'vni']} when={on} />
        <ToggleRow path="typing.modern_tone" title={t('typing.modern.title')} desc={t('typing.modern.desc')} when={on} />
        <ToggleRow path="typing.english_restore" title={t('typing.english.title')} desc={t('typing.english.desc')} when={on} />
      </Card>

      <SectionTitle>{t('sec.typing.apps')}</SectionTitle>
      <Card>
        <Row title={t('typing.apps.title')} desc={t('typing.apps.desc')} when={on}>
          <AddAppButton purpose="typing" label={t('typing.apps.add')} />
        </Row>
        <AppList apps={ty.apps} empty={t('typing.apps.empty')}>
          {(app) => (
            <AppRow app={app}>
              <IconButton icon="close" label={`${t('apps.remove')}: ${app.name}`} onClick={() => send({ t: 'action', name: `typing_app_remove:${app.id}` })} />
            </AppRow>
          )}
        </AppList>
      </Card>
      <Card>
        <InfoRow title={t('typing.engine.title')} desc={t('typing.engine.desc')} />
      </Card>
    </>
  );
}
