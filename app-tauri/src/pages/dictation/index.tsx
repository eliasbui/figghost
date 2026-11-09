import { ActionButton, SegRow, ToggleRow } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { useSection, useT } from '@/stores/hooks';
import './dictation.css';

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
    <Row title={t(`dictation.${name}.title`)} desc={t(`dictation.${name}.desc`)} when={on}>
      <div className="dictation-access">
        <span className="row-desc dictation-status">{t(`dictation.access.${now}`)}</span>
        {now === 'ask' && ask ? <ActionButton name={ask}>{t('dictation.allow')}</ActionButton> : null}
        {now === 'no' || (now === 'ask' && !ask) ? <ActionButton name={open}>{t('dictation.open')}</ActionButton> : null}
      </div>
    </Row>
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
      <Row title={t(`dictation.model.${code}`)} when={on}>
        <span className="row-desc">{t(`dictation.model.${m.on_device ? 'local' : m.available ? 'server' : 'none'}`)}</span>
      </Row>
    );
  };
  return (
    <>
      <Card>
        <Row title={t('dictation.shortcut.title')} desc={t('dictation.shortcut.desc')} when={on}>
          <HotkeyButton target="dictation" allowClear label={t('dictation.start')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.dictation.language')}</SectionTitle>
      <Card>
        <SegRow path="dictation.language" title={t('dictation.language.title')} desc={t('dictation.language.desc')} values={['auto', 'vi', 'en']} when={on} />
        {model('vi')}
        {model('en')}
        <ToggleRow path="dictation.on_device" title={t('dictation.on_device.title')} desc={t('dictation.on_device.desc')} when={on} />
      </Card>

      <SectionTitle>{t('sec.dictation.listening')}</SectionTitle>
      <Card>
        <SegRow path="dictation.silence_seconds" title={t('dictation.silence_seconds.title')} desc={t('dictation.silence_seconds.desc')} values={[0, 2, 3, 5]} when={on} />
        <ToggleRow path="dictation.sound" title={t('dictation.sound.title')} desc={t('dictation.sound.desc')} when={on} />
      </Card>

      <SectionTitle>{t('sec.dictation.access')}</SectionTitle>
      <Card>
        <AccessRow name="speech" now={speech} ask="dictation_allow" open="dictation_open_speech" on={on} />
        <AccessRow name="microphone" now={microphone} ask="dictation_allow" open="dictation_open_microphone" on={on} />
        <AccessRow name="accessibility" now={d.accessibility ? 'yes' : 'no'} ask={null} open="dictation_open_accessibility" on={on} />
      </Card>
    </>
  );
}
