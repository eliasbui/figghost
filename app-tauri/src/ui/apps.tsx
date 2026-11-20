import { Fragment, useEffect } from 'react';
import type { ReactNode } from 'react';
import { DropdownMenu } from '@astryxdesign/core/DropdownMenu';
import { HStack } from '@astryxdesign/core/HStack';
import { ListItem } from '@astryxdesign/core/List';
import { Text } from '@astryxdesign/core/Text';
import type { AppState, Outbound, Purpose } from '@/bridge/messages';
import { patch, send, useApp, wantIcon } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { Select } from './controls';
import { glyph, Icon } from './Icon';
import { IconButton } from './controls';

interface AppRef {
  id: string;
  name: string;
}
interface Rule extends AppRef {
  smooth: boolean | null;
  reverse: boolean | null;
}

/** The icon of an app, asked of the app for in a batch and drawn once it arrives; a dashed frame stands for it till then. */
export function AppIcon({ id }: { id: string }) {
  const src = useApp((s) => s.icons[id]);
  useEffect(() => {
    wantIcon(id);
  }, [id]);
  if (!src)
    return (
      <span className="app-icon blank" data-id={id}>
        <Icon name="app.dashed" />
      </span>
    );
  return <img className="app-icon" alt="" src={src} data-id={id} />;
}

/** One app in a list: its icon, its name, and what can be done with it. */
export function AppRow({ app, children }: { app: AppRef; children?: ReactNode }) {
  return (
    <ListItem
      label={app.name}
      description={<span className="sr-only">{app.id}</span>}
      startContent={<AppIcon id={app.id} />}
      endContent={children ? <HStack gap={2} align="center">{children}</HStack> : undefined}
    />
  );
}

/** What each list of apps already holds, so that the menu offers only the others. */
const LISTED: Record<Purpose, (state: AppState) => string[]> = {
  scroll: (s) => (s.mouse.apps as AppRef[]).map((a) => a.id),
  typing: (s) => (s.typing.apps as AppRef[]).map((a) => a.id),
  ignore: (s) => (s.clipboard.ignored as AppRef[]).map((a) => a.id),
};

/** Vietnamese typing keeps its list itself, through an action that carries the app. */
function addMessage(purpose: Purpose, id: string): Outbound {
  if (purpose === 'typing') return { t: 'action', name: `typing_app_add:${id}` };
  return purpose === 'scroll' ? { t: 'app_add', id } : { t: 'ignored_add', id };
}

/** A button that offers the apps that are open, and a way to choose one from the disk. The apps come from the app a moment after
 *  the button is pressed, and the menu fills in when they do. */
export function AddAppButton({ purpose, label }: { purpose: Purpose; label: string }) {
  const t = useT();
  const reply = useApp((s) => s.running);
  // A string, not a set: a selector that makes a new object on every call never settles.
  const listedIds = useApp((s) => LISTED[purpose](s.state).join('\n'));
  const listed = new Set(listedIds.split('\n'));
  const apps = reply && reply.purpose === purpose ? reply.apps.filter((a) => !listed.has(a.id)) : [];
  return (
    <DropdownMenu
      button={{ label, variant: 'secondary', size: 'sm', icon: glyph('plus') }}
      hasChevron={false}
      placement="below"
      alignment="end"
      onClick={() => send({ t: 'running', purpose })}
      items={[
        ...(apps.length
          ? apps.map((a) => ({ id: a.id, label: a.name || a.id, onClick: () => send(addMessage(purpose, a.id)) }))
          : [{ id: 'none', label: t('menu.none'), isDisabled: true }]),
        { type: 'divider' as const },
        { id: 'disk', label: t('menu.disk'), onClick: () => send({ t: 'pick_app', purpose }) },
      ]}
    />
  );
}

/** An app with its own rule for scrolling: smooth or not, reversed or not, or as the global setting says. */
export function RuleRow({ rule }: { rule: AppRef }) {
  const t = useT();
  const entry = useApp((s) => (s.state.mouse.apps as Rule[]).find((a) => a.id === rule.id));

  const choice = (field: 'smooth' | 'reverse', label: string) => {
    const now = entry?.[field];
    return (
      <HStack gap={1} align="center">
        <Text type="supporting">{label}</Text>
        <Select
          small
          label={`${label}: ${rule.name}`}
          value={now === true ? 'on' : now === false ? 'off' : 'default'}
          options={[
            { value: 'default', label: t('rule.default') },
            { value: 'on', label: t('rule.on') },
            { value: 'off', label: t('rule.off') },
          ]}
          onChange={(v) => {
            const value = v === 'on' ? true : v === 'off' ? false : null;
            patch('mouse.apps', (useApp.getState().state.mouse.apps as Rule[]).map((a) => (a.id === rule.id ? { ...a, [field]: value } : a)));
            send({ t: 'app_rule', id: rule.id, field, value });
          }}
        />
      </HStack>
    );
  };

  return (
    <AppRow app={rule}>
      {choice('smooth', t('apps.smooth'))}
      {choice('reverse', t('apps.reverse'))}
      <IconButton icon="close" label={`${t('apps.remove')}: ${rule.name}`} onClick={() => send({ t: 'app_remove', id: rule.id })} />
    </AppRow>
  );
}

/** An app whose copies the clipboard never records. */
export function IgnoredRow({ app }: { app: AppRef }) {
  const t = useT();
  return (
    <AppRow app={app}>
      <IconButton icon="close" label={`${t('apps.remove')}: ${app.name}`} onClick={() => send({ t: 'ignored_remove', id: app.id })} />
    </AppRow>
  );
}

/** A list of apps, or the line that says there are none. */
export function AppList({ apps, empty, children }: { apps: AppRef[]; empty: string; children: (app: AppRef) => ReactNode }) {
  return apps.length ? <>{apps.map((a) => <Fragment key={a.id}>{children(a)}</Fragment>)}</> : <ListItem label={<Text type="supporting">{empty}</Text>} />;
}
