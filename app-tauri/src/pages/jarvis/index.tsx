import { useEffect, useRef, useState } from 'react';
import { ActionButton, Button, SegRow, SelectRow, ToggleRow } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { change, send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './jarvis.css';

type Access = 'yes' | 'no' | 'ask';

interface JarvisState {
  enabled: boolean;
  provider: string;
  model: string;
  default_model: string;
  needs_key: boolean;
  key: boolean;
  signin?: string;
  account?: { email?: string; plan?: string; error?: string };
  speech: string;
  microphone: boolean;
}

const PROVIDERS = ['anthropic', 'codex', 'openai', 'gemini', 'deepseek', 'local'];

/** The name of the model; it goes to the app when the field is left. */
function ModelField({ jarvis }: { jarvis: JarvisState }) {
  const t = useT();
  const [text, setText] = useState(jarvis.model);
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setText(jarvis.model);
  }, [jarvis.model]);
  return (
    <input
      type="text"
      className="text-field jarvis-model"
      aria-label={t('jarvis.model.title')}
      maxLength={80}
      spellCheck={false}
      autoComplete="off"
      autoCapitalize="off"
      placeholder={jarvis.default_model}
      value={text}
      onFocus={() => {
        focused.current = true;
      }}
      onChange={(e) => setText(e.target.value)}
      onBlur={() => {
        focused.current = false;
        change('jarvis.model', text.trim());
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter') e.currentTarget.blur();
      }}
    />
  );
}

/** The key of the service, or for the ChatGPT plan the sign-in in the browser, with the account it is and a way out. */
function KeyRow({ jarvis, on }: { jarvis: JarvisState; on: boolean }) {
  const t = useT();
  const [value, setValue] = useState('');
  const needed = jarvis.needs_key;
  const state = jarvis.signin;
  const account = jarvis.account ?? {};
  let status: string;
  if (state) {
    status = t(`jarvis.signin.${state}`);
    if (state === 'ready' && account.email) status += `: ${account.email}${account.plan ? ` (${account.plan})` : ''}`;
    if (state !== 'ready' && state !== 'signing' && account.error) status = t('jarvis.signin.failed') + account.error;
  } else {
    status = t(!needed ? 'jarvis.key.notneeded' : jarvis.key ? 'jarvis.key.saved' : 'jarvis.key.none');
  }
  const save = () => {
    const trimmed = value.trim();
    if (!trimmed) return;
    send({ t: 'set', path: 'jarvis.key', value: trimmed });
    setValue('');
  };
  return (
    <Row title={t('jarvis.key.title')} desc={t('jarvis.key.desc')} when={on}>
      <div className="jarvis-key">
        <span className="row-desc jarvis-key-status">{status}</span>
        {needed ? (
          <input
            type="password"
            className="text-field"
            aria-label={t('jarvis.key.title')}
            placeholder={t('jarvis.key.ph')}
            maxLength={400}
            spellCheck={false}
            autoComplete="off"
            autoCapitalize="off"
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') save();
            }}
          />
        ) : null}
        {needed ? (
          <Button small onClick={save}>
            {t('jarvis.key.save')}
          </Button>
        ) : null}
        {needed && jarvis.key ? (
          <Button small onClick={() => send({ t: 'set', path: 'jarvis.key', value: '' })}>
            {t('jarvis.key.remove')}
          </Button>
        ) : null}
        {state === 'missing' || state === 'expired' ? <ActionButton name="jarvis_codex_sign_in">{t('jarvis.signin.in')}</ActionButton> : null}
        {state === 'signing' ? <ActionButton name="jarvis_codex_cancel">{t('jarvis.signin.cancel')}</ActionButton> : null}
        {state === 'ready' ? <ActionButton name="jarvis_codex_sign_out">{t('jarvis.signin.out')}</ActionButton> : null}
      </div>
    </Row>
  );
}

/** One row of access: what it is now, and the one button that moves it on. */
function AccessRow({ name, now, ask, open, on }: { name: string; now: Access; ask: string; open: string; on: boolean }) {
  const t = useT();
  return (
    <Row title={t(`jarvis.${name}.title`)} desc={t(`jarvis.${name}.desc`)} when={on}>
      <div className="jarvis-key">
        <span className="row-desc jarvis-key-status">{t(`jarvis.access.${now}`)}</span>
        {now === 'ask' ? <ActionButton name={ask}>{t('jarvis.allow')}</ActionButton> : null}
        {now === 'no' ? <ActionButton name={open}>{t('jarvis.open')}</ActionButton> : null}
      </div>
    </Row>
  );
}

/** Jarvis (beta): an assistant that answers questions in a chat on Liquid Glass. A shortcut opens the panel. */
export default function Jarvis() {
  const t = useT();
  const jarvis = useSection<JarvisState>('jarvis');
  const on = jarvis.enabled;
  const speech: Access = jarvis.speech === 'allowed' ? 'yes' : jarvis.speech === 'denied' ? 'no' : 'ask';
  const microphone: Access = jarvis.microphone ? 'yes' : jarvis.speech === 'ask' ? 'ask' : 'no';
  return (
    <>
      <Card>
        <Row title={t('jarvis.shortcut.title')} desc={t('jarvis.shortcut.desc')} when={on}>
          <HotkeyButton target="jarvis" allowClear label={t('jarvis.start')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.jarvis.service')}</SectionTitle>
      <Card>
        <SelectRow
          path="jarvis.provider"
          title={t('jarvis.provider.title')}
          desc={t('jarvis.provider.desc')}
          when={on}
          options={PROVIDERS.map((v) => ({ value: v, label: t(`jarvis.provider.${v}`) }))}
        />
        <Row title={t('jarvis.model.title')} desc={t('jarvis.model.desc')} when={on}>
          <ModelField jarvis={jarvis} />
        </Row>
        <KeyRow jarvis={jarvis} on={on} />
      </Card>
      {/* Small print under the service: what the Codex route is, and that it is not an official one. */}
      {on && jarvis.provider === 'codex' ? <p className="jarvis-note">{t('jarvis.signin.note')}</p> : null}

      <SectionTitle>{t('sec.jarvis.voice')}</SectionTitle>
      <Card>
        <ToggleRow path="jarvis.voice" title={t('jarvis.voice.title')} desc={t('jarvis.voice.desc')} when={on} />
        <SegRow path="jarvis.language" title={t('jarvis.language.title')} desc={t('jarvis.language.desc')} values={['auto', 'vi', 'en']} when={on} />
        <ToggleRow path="jarvis.on_device" title={t('jarvis.on_device.title')} desc={t('jarvis.on_device.desc')} when={on} />
        <ToggleRow path="jarvis.speak" title={t('jarvis.speak.title')} desc={t('jarvis.speak.desc')} when={on} />
        <ToggleRow path="jarvis.sound" title={t('jarvis.sound.title')} desc={t('jarvis.sound.desc')} when={on} />
      </Card>

      <SectionTitle>{t('sec.jarvis.access')}</SectionTitle>
      <Card>
        <AccessRow name="speech" now={speech} ask="jarvis_allow" open="jarvis_open_speech" on={on} />
        <AccessRow name="microphone" now={microphone} ask="jarvis_allow" open="jarvis_open_microphone" on={on} />
      </Card>
    </>
  );
}
