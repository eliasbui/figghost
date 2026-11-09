import { Button } from '@/components/macos/Controls';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { go, useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';

/** A key that stays as it is: what it does, and the keys. */
function Cheat({ keys, text }: { keys: string[]; text: string }) {
  return (
    <div className="cheat">
      <span>{text}</span>
      <span className="keys">
        {keys.map((k) => (
          <kbd key={k}>{k}</kbd>
        ))}
      </span>
    </div>
  );
}

// The glyphs of return, the two arrows, command and delete.
const RETURN = '\u21a9';
const UP = '\u2191';
const DOWN = '\u2193';
const COMMAND = '\u2318';
const DELETE = '\u232b';

export default function Keys() {
  const t = useT();
  const state = useApp((s) => s.state);
  // The shortcuts that work anywhere, each also on its own page.
  const global: Array<[target: string, title: string, allowClear: boolean]> = [
    ['clipboard', t('clip.hotkey.title'), false],
    ['notch', t('notch.shortcut.title'), true],
    ...['smart', 'translate', 'vision', 'pin']
      .filter((id) => state[id])
      .map((id): [string, string, boolean] => [id, t(`nav.${id}`), id !== 'smart' && id !== 'translate']),
  ];
  const key = (action: string, title: string, desc?: string) => (
    <Row title={title} desc={desc}>
      <HotkeyButton target={`keys:${action}`} label={title} />
    </Row>
  );

  return (
    <>
      <SectionTitle>{t('keys.sec.panel')}</SectionTitle>
      <Card>
        {key('pin', t('key.pin'))}
        {key('board', t('keys.board.title'), t('keys.board.desc'))}
        {key('paste_other', t('key.pastealt'), t('keys.pastealt.desc'))}
        {key('delete', t('keys.delete.title'), t('keys.delete.desc'))}
      </Card>

      <SectionTitle info={t('keys.fixed.desc')}>{t('keys.sec.fixed')}</SectionTitle>
      <section className="card">
        <div className="cheats">
          <Cheat keys={[RETURN]} text={t('keys.fixed.enter')} />
          <Cheat keys={[UP, DOWN]} text={t('keys.fixed.move')} />
          <Cheat keys={[COMMAND, '1-9']} text={t('keys.fixed.tab')} />
          <Cheat keys={[COMMAND, 'A-Z']} text={t('key.pinkey')} />
          <Cheat keys={[COMMAND, DELETE]} text={t('key.delete')} />
          <Cheat keys={['Esc']} text={t('key.close')} />
        </div>
      </section>

      <SectionTitle>{t('keys.sec.global')}</SectionTitle>
      <Card>
        {global.map(([target, title, allowClear]) => (
          <Row key={target} title={title}>
            <HotkeyButton target={target} label={title} allowClear={allowClear} />
          </Row>
        ))}
        {['capture', 'tiling'].map((id) => (
          <Row key={id} title={t(`nav.${id}`)} desc={t('keys.elsewhere')}>
            <Button small onClick={() => go(id)}>
              {t('keys.open')}
            </Button>
          </Row>
        ))}
      </Card>
    </>
  );
}
