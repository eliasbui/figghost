// How a page of settings is laid out: names of groups, cards of rows, a state said in a word. Astryx's own parts, with the
// words of a row kept the way the pages already hand them over (a title, and what it would say behind an "i").
import { createContext, useContext } from 'react';
import type { ReactNode } from 'react';
import { Badge } from '@astryxdesign/core/Badge';
import { Card as AstryxCard } from '@astryxdesign/core/Card';
import { Heading } from '@astryxdesign/core/Heading';
import { HStack } from '@astryxdesign/core/HStack';
import { InfoTip } from '@astryxdesign/core/InfoTip';
import { List, ListItem } from '@astryxdesign/core/List';
import { VStack } from '@astryxdesign/core/VStack';
import { cn } from '@/lib/cn';

/** True while the page's search reads the text of every page: what a row keeps behind its "i" is then written out, and the
 *  groups behind Advanced count too. */
export const IndexingContext = createContext(false);

/** The name of a row. What the row would say in words is kept behind an "i" beside it. */
export function Name({ title, desc }: { title: ReactNode; desc?: ReactNode }) {
  const indexing = useContext(IndexingContext);
  if (!desc) return <>{title}</>;
  return (
    <HStack gap={1} align="center">
      {title}
      <InfoTip content={desc} />
      {indexing ? <span hidden>{desc}</span> : null}
    </HStack>
  );
}

/** The name of a group of rows. What the group would say in words is kept behind an "i" after the name. */
export function SectionTitle({ children, minor, info }: { children: ReactNode; minor?: boolean; info?: ReactNode }) {
  return (
    <HStack gap={1} align="center" className="section-title">
      <Heading level={minor ? 5 : 4}>{children}</Heading>
      {info ? <InfoTip content={info} /> : null}
    </HStack>
  );
}

export interface RowProps {
  title: ReactNode;
  /** What the row would say in words. It is kept behind the "i" beside the name; a row with nothing to set says it in the open. */
  desc?: ReactNode;
  /** More under the name, such as a line that says how things stand. */
  below?: ReactNode;
  /** A picture before the name. */
  lead?: ReactNode;
  /** The control at the right of the row. */
  children?: ReactNode;
  /** The control takes a line of its own under the text. */
  block?: boolean;
  /** False greys the row and takes it out of reach: what it sets means nothing while another setting is off. */
  when?: boolean;
  /** A state the style sheet of a page colours the row's parts by (`data-state`). */
  state?: string;
  className?: string;
}

/** A name and one control, on one line: the name at the left, the control at the right. */
export function Row({ title, desc, below, lead, children, block, when = true, state, className }: RowProps) {
  const control = children !== undefined && children !== null && children !== false;
  const named = title !== undefined && title !== null && title !== false && title !== '';
  const explains = !control && !below && !!desc;
  // A row that is off is taken out of reach as a whole (`inert`) and drawn dim: Astryx's own disabled state is the row's, not
  // that of the controls in it.
  const off = when ? undefined : 'dim';
  const under = control && block ? (
    <>
      {below}
      <div className={cn('row-block', off)} inert={!when}>
        {children}
      </div>
    </>
  ) : explains ? desc : below;
  return (
    <ListItem
      label={named ? <Name title={title} desc={explains ? undefined : desc} /> : ''}
      description={under || undefined}
      startContent={lead}
      endContent={
        control && !block ? (
          <div className={cn('row-end', off)} inert={!when}>
            <HStack gap={2} align="center">
              {children}
            </HStack>
          </div>
        ) : undefined
      }
      isDisabled={!when}
      className={className}
      data-state={state}
    />
  );
}

/** A line of a card that the page fills with whatever it draws: a table, a field with its button, a list of its own. */
export function Block({ children, className }: { children?: ReactNode; className?: string }) {
  return <li className={cn('block-row', className)}>{children}</li>;
}

/** A group of rows on one pane, its lines divided from each other. */
export function Card({ children, className }: { children?: ReactNode; className?: string }) {
  return (
    <AstryxCard padding={0} className={className}>
      <List hasDividers>{children}</List>
    </AstryxCard>
  );
}

/** A group of rows under its name. A group with no name is the card alone. */
export function Section({ title, children }: { title?: ReactNode; children: ReactNode }) {
  return (
    <VStack gap={2}>
      {title ? <SectionTitle>{title}</SectionTitle> : null}
      <Card>{children}</Card>
    </VStack>
  );
}

const TONE = { good: 'success', warn: 'warning', bad: 'error', off: 'neutral' } as const;

/** A state said in a word: orange for what is still to do, green once it is done. */
export function Pill({ children, good, tone }: { children: ReactNode; good?: boolean; tone?: 'good' | 'warn' | 'bad' | 'off' }) {
  return <Badge variant={TONE[tone ?? (good ? 'good' : 'warn')]} label={children} />;
}
