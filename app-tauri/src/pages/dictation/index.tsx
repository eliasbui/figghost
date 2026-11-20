import { HStack } from '@astryxdesign/core/HStack';
import { ListItem } from '@astryxdesign/core/List';
import { Text } from '@astryxdesign/core/Text';
import { VStack } from '@astryxdesign/core/VStack';
import { ActionButton } from '@/ui/controls';
import { HotkeyButton } from '@/ui/hotkey';
import { Name, Section } from '@/ui/layout';
import { SegRow, ToggleRow } from '@/ui/rows';
import { useSection, useT } from '@/stores/hooks';

type Access = 'yes' | 'no' | 'ask';

interface DictationState {
  enabled: boolean;
  speech: string;
  microphone: boolean;
  accessibility: boolean;
  models: Record<string, { on_device: boolean; available: boolean }>;
}

/** One row of access: what it is now, and the one button that moves it on. */
function AccessRow({ name, now, ask, open, on }: { name: string; now: Access; ask: string | null; open: string; on: boolean }) {
  const t = useT();
  return (
    <ListItem
      label={<Name title={t(`dictation.${name}.title`)} desc={t(`dictation.${name}.desc`)} />}
      isDisabled={!on}
      endContent={
        <HStack gap={2} align="center" wrap="wrap">
          <Text type="supporting">{t(`dictation.access.${now}`)}</Text>
          {now === 'ask' && ask ? <ActionButton name={ask} label={t('dictation.allow')} /> : null}
          {now === 'no' || (now === 'ask' && !ask) ? <ActionButton name={open} label={t('dictation.open')} /> : null}
        </HStack>
      }
    />
  );
}

/** Dictation: speak into any text box. A shortcut brings a microphone to the middle of the screen, and the words go in. */
export default function Dictation() {
  const t = useT();
  const d = useSection<DictationState>('dictation');
  const on = d.enabled;
  const speech: Access = d.speech === 'allowed' ? 'yes' : d.speech === 'denied' ? 'no' : 'ask';
  const microphone: Access = d.microphone ? 'yes' : d.speech === 'ask' ? 'ask' : 'no';
  const model = (code: string) => {
    const m = d.models[code];
    return (
      <ListItem
        label={t(`dictation.model.${code}`)}
        isDisabled={!on}
        endContent={<Text type="supporting">{t(`dictation.model.${m.on_device ? 'local' : m.available ? 'server' : 'none'}`)}</Text>}
      />
    );
  };
  return (
    <VStack gap={6}>
      <Section>
        <ListItem
          label={<Name title={t('dictation.shortcut.title')} desc={t('dictation.shortcut.desc')} />}
          isDisabled={!on}
          endContent={<HotkeyButton target="dictation" allowClear label={t('dictation.start')} />}
        />
      </Section>

      <Section title={t('sec.dictation.language')}>
        <SegRow path="dictation.language" title={t('dictation.language.title')} desc={t('dictation.language.desc')} values={['auto', 'vi', 'en']} when={on} />
        {model('vi')}
        {model('en')}
        <ToggleRow path="dictation.on_device" title={t('dictation.on_device.title')} desc={t('dictation.on_device.desc')} when={on} />
      </Section>

      <Section title={t('sec.dictation.listening')}>
        <SegRow path="dictation.silence_seconds" title={t('dictation.silence_seconds.title')} desc={t('dictation.silence_seconds.desc')} values={[0, 2, 3, 5]} when={on} />
        <ToggleRow path="dictation.sound" title={t('dictation.sound.title')} desc={t('dictation.sound.desc')} when={on} />
      </Section>

      <Section title={t('sec.dictation.access')}>
        <AccessRow name="speech" now={speech} ask="dictation_allow" open="dictation_open_speech" on={on} />
        <AccessRow name="microphone" now={microphone} ask="dictation_allow" open="dictation_open_microphone" on={on} />
        <AccessRow name="accessibility" now={d.accessibility ? 'yes' : 'no'} ask={null} open="dictation_open_accessibility" on={on} />
      </Section>
    </VStack>
  );
}
