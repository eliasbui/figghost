import { Button, CodeBlock, Select, ToggleRow } from '@/components/macos/Controls';
import { AddAppButton, AppList, IgnoredRow } from '@/components/macos/Apps';
import { Advanced, confirmDialog } from '@/components/macos/Dialogs';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { keysText } from '@/lib/shortcuts';
import { cx } from '@/lib/text';
import { change, send, showToast } from '@/stores/app';
import { useSetting, useT } from '@/stores/hooks';
import { MascotCards } from './Mascot';
import { StackCards } from './Stack';

const LIMITS = [100, 250, 500, 1000, 5000, 20000];

/** How an AI assistant starts the clipboard server, as the JSON it keeps in its own configuration. */
function McpCard() {
  const t = useT();
  const enabled = useSetting<boolean>('mcp.enabled');
  const command = useSetting<string>('mcp.command');
  const snippet = JSON.stringify({ mcpServers: { figghost: { command, args: ['--mcp'] } } }, null, 2);
  return (
    <Card>
      <ToggleRow path="mcp.enabled" title={t('mcp.title')} desc={t('mcp.desc')} />
      <div className={cx('row block', !enabled && 'dim')}>
        <div className="row-desc">{t('mcp.how')}</div>
        <CodeBlock label={t('mcp.how')}>{snippet}</CodeBlock>
        <div>
          <Button
            small
            onClick={() => {
              send({ t: 'action', name: 'copy_mcp_config' });
              showToast(t('mcp.copied'));
            }}
          >
            {t('mcp.copy')}
          </Button>
        </div>
      </div>
    </Card>
  );
}

export default function Clipboard() {
  const t = useT();
  const limit = useSetting<number>('clipboard.history_limit');
  const count = useSetting<number>('clipboard.count');
  const ignored = useSetting<Array<{ id: string; name: string }>>('clipboard.ignored');
  const pasteOther = useSetting<string>('keys.shortcuts.paste_other');
  const limits = LIMITS.includes(limit) ? LIMITS : [...LIMITS, limit];

  return (
    <>
      <SectionTitle>{t('sec.pasting')}</SectionTitle>
      <Card>
        <Row title={t('clip.hotkey.title')} desc={t('clip.hotkey.desc')}>
          <HotkeyButton target="clipboard" label={t('clip.hotkey.title')} />
        </Row>
        <ToggleRow path="clipboard.auto_paste" title={t('paste.auto.title')} desc={t('paste.auto.desc')} />
        <ToggleRow path="clipboard.paste_plain" title={t('paste.plain.title')} desc={t('paste.plain.desc', { key: keysText(pasteOther) })} />
      </Card>

      <SectionTitle>{t('sec.history')}</SectionTitle>
      <Card>
        <Row title={t('limit.title')} desc={t('limit.desc')}>
          <Select label={t('limit.title')} value={limit} options={limits.map((v) => ({ value: v, label: v.toLocaleString() }))} onChange={(v) => change('clipboard.history_limit', v)} />
        </Row>
        <Row title={t('count.title')}>
          <span className="row-title">{count.toLocaleString()}</span>
        </Row>
        <Row title={t('clear.title')} desc={t('clear.desc')}>
          <Button
            small
            danger
            icon="trash"
            onClick={() =>
              confirmDialog({ title: t('clear.confirm.title'), body: t('clear.confirm.body'), confirm: t('clear.btn') }, () => send({ t: 'action', name: 'clear_history' }))
            }
          >
            {t('clear.btn')}
          </Button>
        </Row>
      </Card>

      <SectionTitle>{t('sec.privacy')}</SectionTitle>
      <Card>
        <Row title={t('ignored.title')} desc={t('ignored.desc')}>
          <AddAppButton purpose="ignore" label={t('ignored.add')} />
        </Row>
        <AppList apps={ignored} empty={t('ignored.empty')}>
          {(app) => <IgnoredRow app={app} />}
        </AppList>
      </Card>

      <StackCards />

      <Advanced>
        <SectionTitle>{t('sec.ai')}</SectionTitle>
        <McpCard />
        <MascotCards />
      </Advanced>
    </>
  );
}
