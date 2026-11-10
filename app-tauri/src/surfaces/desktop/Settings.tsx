import { useRef } from 'react';
import { post } from './bridge';
import type { State } from './logic';
import { Btn, Card, Grow, Header, Hint, Input, Mono, Row, Section, Select, Toggle } from './ui';

interface Props {
  t: (key: string, vars?: Record<string, string | number>) => string;
  state: State;
}

const set = (key: string, value: unknown) => post({ t: 'set', key, value });

export function SettingsPage({ t, state }: Props) {
  const { config: c, env } = state;
  const hotkey = useRef<HTMLInputElement>(null);
  const options = (ids: string[]) => ids.map((id) => ({ id, text: t(`opt.${id}`) }));
  return (
    <>
      <Header title={t('page.settings')} sub={t('settings.sub')} />
      <Section>{t('settings.general')}</Section>
      <Card>
        <Row>
          <Grow>{t('settings.language')}</Grow>
          <Select label={t('settings.language')} value={c.language} options={options(['auto', 'en', 'vi'])} onChange={(v) => set('language', v)} />
        </Row>
        <Row>
          <Grow>{t('settings.theme')}</Grow>
          <Select label={t('settings.theme')} value={c.theme} options={options(['auto', 'light', 'dark'])} onChange={(v) => set('theme', v)} />
        </Row>
        <Row>
          <Grow>
            <div>{t('settings.autostart')}</div>
            {env.autostart === null ? <Hint>{t('settings.autostart.na')}</Hint> : null}
          </Grow>
          {env.autostart === null ? null : <Toggle checked={env.autostart} label={t('settings.autostart')} onChange={(on) => post({ t: 'autostart', value: on })} />}
        </Row>
      </Card>
      <Section>{t('settings.clipboard')}</Section>
      <Card>
        <Row>
          <Grow>
            <div>{t('settings.hotkey')}</div>
            <Hint>{env.hotkey_error ? t('error', { e: env.hotkey_error }) : t('settings.hotkey.hint')}</Hint>
          </Grow>
          <Input ref={hotkey} key={c.hotkey} type="text" defaultValue={c.hotkey} aria-label={t('settings.hotkey')} spellCheck={false} />
          <Btn onClick={() => set('hotkey', (hotkey.current?.value ?? '').trim())}>{t('settings.save')}</Btn>
        </Row>
        <Row>
          <Grow>{t('settings.limit')}</Grow>
          <Input type="number" min={10} max={100000} className="w-28" aria-label={t('settings.limit')} defaultValue={c.history_limit} key={c.history_limit} onChange={(event) => set('history_limit', Number(event.target.value))} />
        </Row>
        <Row>
          <Grow>{t('settings.secrets')}</Grow>
          <Select label={t('settings.secrets')} value={c.secrets} options={options(['keep', 'mask', 'skip'])} onChange={(v) => set('secrets', v)} />
        </Row>
        <Row>
          <Grow>
            <div>{t('settings.mcp')}</div>
            {c.mcp.enabled ? (
              <>
                <Hint>{t('settings.mcp.hint')}</Hint>
                <Mono>{`"${env.exe}" --mcp`}</Mono>
              </>
            ) : null}
          </Grow>
          <Toggle checked={c.mcp.enabled} label={t('settings.mcp')} onChange={(on) => set('mcp.enabled', on)} />
        </Row>
      </Card>
      <div className="h-6" />
      <Card>
        <Row>
          <Grow>
            <div>{t('settings.data')}</div>
            <Mono className="mt-1">{env.data_dir}</Mono>
          </Grow>
          <Btn onClick={() => post({ t: 'open_data' })}>{t('settings.open')}</Btn>
        </Row>
      </Card>
    </>
  );
}
