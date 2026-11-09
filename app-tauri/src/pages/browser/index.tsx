import { useEffect, useRef, useState } from 'react';
import { ActionButton, Button, SelectRow, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { Card, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './browser.css';

interface Browser {
  id: string;
  name: string;
  found: boolean;
  installed: boolean;
}
interface BrowserState {
  enabled: boolean;
  connected: boolean;
  browsers?: Browser[];
  sites: string;
  blocking: boolean;
  speed_control: boolean;
}

/** The sites to keep out of the browsers, one to a line; the text goes to the app when the field is left. */
function Sites({ browser, on }: { browser: BrowserState; on: boolean }) {
  const t = useT();
  const [text, setText] = useState(browser.sites);
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setText(browser.sites);
  }, [browser.sites]);
  return (
    <textarea
      className="browser-sites"
      rows={6}
      aria-label={t('browser.sites.title')}
      spellCheck={false}
      disabled={!on}
      value={text}
      onFocus={() => {
        focused.current = true;
      }}
      onChange={(e) => setText(e.target.value)}
      onBlur={() => {
        focused.current = false;
        if (text !== browser.sites) send({ t: 'set', path: 'browser.sites', value: text });
      }}
    />
  );
}

/** Browser: the Figghost Companion extension for the browsers built on Chromium, and the bridge it talks to. */
export default function BrowserPage() {
  const t = useT();
  const browser = useSection<BrowserState>('browser');
  const on = browser.enabled;
  const found = (browser.browsers ?? []).filter((b) => b.found);
  return (
    <>
      <SectionTitle>{t('sec.browser.setup')}</SectionTitle>
      <Card>
        <Row title={t('browser.folder.title')} desc={t('browser.folder.desc')} when={on}>
          <ActionIcon name="browser_folder" icon="folder" label={t('browser.folder.open')} />
        </Row>
        <Row title={t('browser.bridge.title')} desc={t('browser.bridge.desc')} when={on} />
        {found.length ? (
          found.map((b) => (
            <Row key={b.id} title={b.name} when={on}>
              <Pill good={b.installed}>{t(b.installed ? 'browser.bridge.on' : 'browser.bridge.off')}</Pill>
              <ActionButton name={`${b.installed ? 'browser_remove:' : 'browser_install:'}${b.id}`}>{t(b.installed ? 'browser.bridge.remove' : 'browser.bridge.add')}</ActionButton>
            </Row>
          ))
        ) : (
          <Row title={t('browser.none')} when={on} />
        )}
        <Row title={t('browser.link.title')} when={on}>
          <Pill good={browser.connected}>{t(browser.connected ? 'browser.link.on' : 'browser.link.off')}</Pill>
        </Row>
        <Row title={t('browser.remove.title')} desc={t('browser.remove.desc')}>
          <Button small danger onClick={() => send({ t: 'action', name: 'browser_remove_all' })}>
            {t('browser.remove.all')}
          </Button>
        </Row>
      </Card>

      <SectionTitle>{t('sec.browser.block')}</SectionTitle>
      <Card>
        <ToggleRow path="browser.block" title={t('browser.block.title')} desc={t('browser.block.desc')} when={on} />
        <Row title={t('browser.sites.title')} desc={t('browser.sites.desc')} when={on}>
          <span className="util-state">{browser.blocking ? t('browser.blocking') : ''}</span>
        </Row>
        <Row title="" block when={on}>
          <Sites browser={browser} on={on} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.browser.speed')}</SectionTitle>
      <Card>
        <ToggleRow path="browser.speed_control" title={t('browser.speed.title')} desc={t('browser.speed.desc')} when={on} />
        <SelectRow
          path="browser.speed"
          title={t('browser.speed.pick')}
          when={on && browser.speed_control}
          options={[1, 1.25, 1.5, 1.75, 2].map((n) => ({ value: n, label: t('browser.speed.n', { n }) }))}
        />
      </Card>

      <SectionTitle>{t('sec.browser.download')}</SectionTitle>
      <Card>
        <ToggleRow path="browser.download" title={t('browser.download.title')} desc={t('browser.download.desc')} when={on} />
      </Card>
    </>
  );
}
