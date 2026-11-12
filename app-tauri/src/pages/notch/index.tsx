import { useState } from 'react';
import { Button, IconButton, Segmented, SegRow, SelectRow, SliderRow, SwitchButton, ToggleRow } from '@/components/macos/Controls';
import { Advanced } from '@/components/macos/Dialogs';
import { HotkeyButton } from '@/components/macos/Hotkey';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { PageIcon } from '@/components/macos/Page';
import { hasKey } from '@/i18n';
import { isPage, PAGE_SYMBOLS } from '@/pages/meta';
import { change, openPage, patch, send } from '@/stores/app';
import { NotchPreview } from './NotchPreview';
import type { PreviewNotch } from './NotchPreview';
import { useSection, useT } from '@/stores/hooks';
import type { TFunction } from '@/stores/hooks';

interface NotchTab {
  id: string;
  title: string;
  on: boolean;
  waiting: boolean;
}

interface NotchState {
  enabled: boolean;
  hardware: boolean;
  open_on_hover: boolean;
  hover_delay_ms: number;
  live_media: boolean;
  rest_clock: boolean;
  pages: NotchTab[];
  screens?: string[];
  [size: string]: unknown;
}

// The sizes of the notch as ready-made styles. 0 is automatic: what each tab needs, or the default pill.
const SIZES = ['width', 'height', 'rest_width', 'rest_height'] as const;
const STYLES: Record<string, Record<(typeof SIZES)[number], number>> = {
  compact: { width: 0, height: 0, rest_width: 150, rest_height: 0 },
  standard: { width: 0, height: 0, rest_width: 0, rest_height: 0 },
  wide: { width: 800, height: 520, rest_width: 260, rest_height: 32 },
};
const styleOf = (notch: NotchState) => Object.keys(STYLES).find((k) => SIZES.every((f) => STYLES[k][f] === notch[f])) ?? 'custom';

const DELAYS = [0, 100, 200, 300, 500, 800];

function tabTitle(t: TFunction, tab: NotchTab): string {
  return hasKey(`tab.${tab.id}`) ? t(`tab.${tab.id}`) : tab.title;
}

/** Writes a new list of tabs: the page keeps every tab in order, the app keeps the ids of the ones that show. */
function sendTabs(tabs: NotchTab[]): void {
  patch('notch.pages', tabs);
  send({ t: 'set', path: 'notch.pages', value: tabs.filter((p) => p.on).map((p) => p.id) });
}

/** One tab of the notch: where it stands in the order, whether it shows, and its page. */
function TabRow({ tab, tabs }: { tab: NotchTab; tabs: NotchTab[] }) {
  const t = useT();
  const at = tabs.findIndex((p) => p.id === tab.id);
  const title = tabTitle(t, tab);
  const shows = (p: NotchTab) => p.on && !p.waiting;
  // The last tab that shows cannot go: a notch with no tab could not be used. A tab that waits for its feature does not show.
  const locked = shows(tab) && tabs.filter(shows).length === 1;
  const desc = tab.waiting ? t('tab.waiting') : hasKey(`tab.${tab.id}.desc`) ? t(`tab.${tab.id}.desc`) : null;

  const move = (by: number) => {
    const j = at + by;
    if (at < 0 || j < 0 || j >= tabs.length) return;
    const list = tabs.slice();
    [list[at], list[j]] = [list[j], list[at]];
    sendTabs(list);
  };

  return (
    <Row title={title} desc={desc} lead={PAGE_SYMBOLS[tab.id] ? <PageIcon id={tab.id} /> : null}>
      <span className="tab-order">
        <IconButton icon="chevron.up" size="sm" label={`${t('tab.up')}: ${title}`} disabled={at <= 0} onClick={() => move(-1)} />
        <IconButton icon="chevron.down" size="sm" label={`${t('tab.down')}: ${title}`} disabled={at >= tabs.length - 1} onClick={() => move(1)} />
      </span>
      <SwitchButton
        checked={tab.on}
        disabled={locked}
        label={title}
        onToggle={() => {
          const list = tabs.map((p) => (p.id === tab.id ? { ...p, on: !p.on } : p));
          if (list.some((p) => p.on)) sendTabs(list);
        }}
      />
      {/* A tab that has a page of its own opens it, as System Settings opens the page of one item of a list. */}
      {isPage(tab.id) ? <IconButton className="tab-open" icon="chevron.right" size="sm" label={t('tab.open', { tab: title })} onClick={() => openPage(tab.id)} /> : null}
    </Row>
  );
}

/** The size of the notch as one choice of four; Custom shows a slider for each size. */
function SizeRows({ notch, on }: { notch: NotchState; on: boolean }) {
  const t = useT();
  const [customOpen, setCustomOpen] = useState(false);
  const style = customOpen ? 'custom' : styleOf(notch);
  const closed = on && !notch.hardware;
  const slider = (field: (typeof SIZES)[number], min: number, max: number, step: number, key: string, when?: boolean) => (
    <SliderRow
      path={`notch.${field}`}
      title={t(`${key}.title`)}
      desc={t(`${key}.desc`)}
      min={min}
      max={max}
      step={step}
      when={when}
      format={(v) => (v === 0 ? t('notch.width.auto') : t('notch.width.pt', { n: Math.round(v) }))}
    />
  );
  return (
    <>
      <Row title={t('sec.notch.style')} desc={t(`notch.style.${style}.desc`)} when={on}>
        <Segmented
          label={t('sec.notch.style')}
          value={style}
          options={[...Object.keys(STYLES), 'custom'].map((id) => ({ value: id, label: t(`notch.style.${id}`) }))}
          onPick={(id) => {
            setCustomOpen(id === 'custom');
            if (id !== 'custom') for (const f of SIZES) change(`notch.${f}`, STYLES[id][f]);
          }}
        />
      </Row>
      {style === 'custom' ? (
        <>
          {slider('width', 480, 1000, 10, 'notch.width')}
          {slider('height', 260, 900, 10, 'notch.height')}
          {slider('rest_width', 120, 400, 5, 'notch.restwidth', closed)}
          {slider('rest_height', 20, 48, 1, 'notch.restheight', closed)}
          <Row title={t('notch.style.reset.title')} desc={t('notch.style.reset.desc')}>
            <Button
              small
              onClick={() => {
                setCustomOpen(false);
                for (const f of SIZES) change(`notch.${f}`, 0);
              }}
            >
              {t('notch.style.reset')}
            </Button>
          </Row>
        </>
      ) : null}
    </>
  );
}

export default function Notch() {
  const t = useT();
  const notch = useSection<NotchState>('notch');
  const on = notch.enabled;
  const tabs = notch.pages;
  const shownTabs = tabs.filter((p) => p.on && !p.waiting);
  const delays = DELAYS.includes(notch.hover_delay_ms) ? DELAYS : [...DELAYS, notch.hover_delay_ms];

  return (
    <>
      <NotchPreview notch={notch as unknown as PreviewNotch} />
      <SectionTitle>{t('sec.notch.look')}</SectionTitle>
      <Card>
        <SizeRows notch={notch} on={on} />
        <SelectRow
          path="notch.material"
          title={t('notch.material.title')}
          desc={t('notch.material.desc')}
          when={on}
          options={['glass', 'clear', 'ink', 'accent', 'black'].map((v) => ({ value: v, label: t(`notch.material.${v}`) }))}
        />
        <SegRow path="notch.motion" title={t('notch.motion.title')} desc={t('notch.motion.desc')} values={['spring', 'smooth', 'none']} when={on} />
      </Card>

      <SectionTitle>{t('sec.notch.behavior')}</SectionTitle>
      <Card>
        <ToggleRow path="notch.open_on_hover" title={t('notch.hover.title')} desc={t('notch.hover.desc')} when={on} />
        <SelectRow
          path="notch.hover_delay_ms"
          title={t('notch.delay.title')}
          desc={t('notch.delay.desc')}
          when={on && notch.open_on_hover}
          options={delays.map((n) => ({ value: n, label: n ? t('notch.delay.ms', { n }) : t('notch.delay.none') }))}
        />
        <SelectRow
          path="notch.open_page"
          title={t('notch.openpage.title')}
          desc={t('notch.openpage.desc')}
          when={on}
          options={[
            { value: 'activity', label: t('notch.openpage.activity') },
            { value: 'last', label: t('notch.openpage.last') },
            ...shownTabs.map((p) => ({ value: p.id, label: tabTitle(t, p) })),
          ]}
        />
        <Row title={t('notch.shortcut.title')} desc={t('notch.shortcut.desc')} when={on}>
          <HotkeyButton target="notch" allowClear label={t('notch.shortcut.title')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.notch.closed')}</SectionTitle>
      <Card>
        <ToggleRow path="notch.live_media" title={t('notch.livemedia.title')} desc={t('notch.livemedia.desc')} when={on} />
        <SegRow path="notch.waveform" title={t('notch.waveform.title')} desc={t('notch.waveform.desc')} values={['live', 'animated']} when={on && notch.live_media} />
        <ToggleRow path="notch.live_agents" title={t('notch.liveagents.title')} desc={t('notch.liveagents.desc')} when={on} />
        <ToggleRow path="notch.rest_clock" title={t('notch.restclock.title')} desc={t('notch.restclock.desc')} when={on} />
        <ToggleRow path="notch.rest_steps" title={t('notch.reststeps.title')} desc={t('notch.reststeps.desc')} when={on && notch.rest_clock} />
        <SegRow path="notch.cutout" title={t('notch.cutout.title')} desc={t('notch.cutout.desc')} values={['auto', 'yes', 'no']} when={on} />
      </Card>

      <SectionTitle info={t('tabs.desc')}>{t('sec.tabs')}</SectionTitle>
      <Card>
        {tabs.map((tab) => (
          <TabRow key={tab.id} tab={tab} tabs={tabs} />
        ))}
      </Card>

      <Advanced>
        <SectionTitle>{t('sec.notch.display')}</SectionTitle>
        <Card>
          <SelectRow
            path="notch.display"
            title={t('notch.display.title')}
            desc={t('notch.display.desc')}
            when={on}
            options={[
              ...['auto', 'main', 'pointer'].map((v) => ({ value: v, label: t(`notch.display.${v}`) })),
              ...(notch.screens ?? []).map((name) => ({ value: `screen:${name}`, label: name })),
            ]}
          />
          <SegRow
            path="notch.rest"
            title={t('notch.rest.title')}
            desc={notch.hardware ? t('notch.rest.hardware') : t('notch.rest.desc')}
            values={['notch', 'hidden']}
            when={on && !notch.hardware}
          />
          <ToggleRow path="notch.show_in_fullscreen" title={t('notch.fullscreen.title')} desc={t('notch.fullscreen.desc')} when={on} />
          <ToggleRow path="notch.hide_in_capture" title={t('notch.capture.title')} desc={t('notch.capture.desc')} when={on} />
          <ToggleRow path="notch.haptics" title={t('notch.haptics.title')} desc={t('notch.haptics.desc')} when={on} />
          <ToggleRow path="notch.shadow" title={t('notch.shadow.title')} desc={t('notch.shadow.desc')} when={on} />
        </Card>
      </Advanced>
    </>
  );
}
