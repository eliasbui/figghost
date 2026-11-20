import type { ReactNode } from 'react';
import { Heading } from '@astryxdesign/core/Heading';
import { HStack } from '@astryxdesign/core/HStack';
import { Icon as AstryxIcon } from '@astryxdesign/core/Icon';
import { Text } from '@astryxdesign/core/Text';
import appIcon from '@/assets/app-icon.png';
import { masterPath, PAGE_SYMBOLS } from '@/pages/meta';
import { useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { Switch } from './controls';
import { iconType } from './Icon';

/** The picture of a page. */
export function PageIcon({ id, weight = 'regular' }: { id: string; className?: string; weight?: 'regular' | 'fill' | 'duotone' | 'bold' }) {
  const icon = iconType(PAGE_SYMBOLS[id] ?? 'pg.general', weight);
  return icon ? <AstryxIcon icon={icon} /> : null;
}

/** The app's own icon. */
export function AppLogo({ className, size = 40 }: { className?: string; size?: number }) {
  return <img className={className} src={appIcon} width={size} height={size} alt="" draggable={false} />;
}

/** The top of General and About: the app's own icon, its name, a line, and a control. */
export function AppHead({ line, children }: { line: string; children?: ReactNode }) {
  return (
    <HStack gap={3} align="center" className="hero">
      <AppLogo size={56} />
      <div className="hero-text">
        <Heading level={2}>Figghost</Heading>
        <Text type="supporting">{line}</Text>
      </div>
      {children}
    </HStack>
  );
}

/** The top of every other page: its picture, its name, and the switch of its feature, which a page has no row for. */
export function PageHero({ id }: { id: string }) {
  const t = useT();
  const master = useApp((s) => masterPath(id, s.state));
  return (
    <HStack gap={3} align="center" className="hero">
      <PageIcon id={id} weight="fill" />
      <div className="hero-text">
        <Heading level={2}>{t(`page.${id}`)}</Heading>
        {/* What the page is for: kept in the page for the search and for the reader of the screen. */}
        <span className="sr-only">{t(`page.${id}.sub`)}</span>
      </div>
      {master ? <Switch path={master} label={t(`nav.${id}`)} /> : null}
    </HStack>
  );
}
