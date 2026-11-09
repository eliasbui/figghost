import { Children, isValidElement } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { cx } from '@/lib/text';
import { InfoTip } from './Overlay';

/** A row at the end of a card that only explains, with at least this much to say, is small print under the card. */
const NOTE_FROM = 60;

/** The name of a group of rows. What the group would say in words is kept behind an "i" after the name. */
export function SectionTitle({ children, minor, info }: { children: ReactNode; minor?: boolean; info?: ReactNode }) {
  return (
    <div className={cx('section-title', minor && 'minor')}>
      {children}
      {info ? <InfoTip>{info}</InfoTip> : null}
    </div>
  );
}

export interface RowProps {
  title: ReactNode;
  /** What the row would say in words. It is kept behind the "i" beside the name, and in the page for the search and for the
   *  reader of the screen; it is not drawn. */
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
  /** Small print under a card, which is how a row that only explains is drawn at the end of it. */
  note?: boolean;
  /** A state the style sheet of a page colours the row's parts by (`data-state`). */
  state?: string;
  className?: string;
}

/** A name and one control, on one line of the grid: names at the left, controls at the right. */
export function Row({ title, desc, below, lead, children, block, when = true, note, state, className }: RowProps) {
  const control = children !== undefined && children !== null && children !== false;
  const named = title !== undefined && title !== null && title !== false && title !== '';
  if (note) {
    return (
      <div className={cx('card-note', className)} data-state={state}>
        {desc ? <InfoTip text={title}>{desc}</InfoTip> : <span>{title}</span>}
        {desc ? <span className="row-info sr-only">{desc}</span> : null}
      </div>
    );
  }
  return (
    <div className={cx('row', block && 'block', !when && 'disabled', className)} data-state={state}>
      {lead ? <div className="row-lead">{lead}</div> : null}
      {named || below ? (
        <div className="row-text">
          {named ? (
            <div className="row-title">
              <span className="row-name">{title}</span>
              {desc ? <InfoTip>{desc}</InfoTip> : null}
            </div>
          ) : null}
          {desc ? <span className="row-info sr-only">{desc}</span> : null}
          {below ? <div className="row-sub">{below}</div> : null}
        </div>
      ) : desc ? (
        <span className="row-info sr-only">{desc}</span>
      ) : null}
      {control ? <div className="row-control">{children}</div> : null}
    </div>
  );
}

/** A row with nothing to set in it. */
export const InfoRow = Row;

function explains(node: unknown): node is ReactElement<RowProps> {
  if (!isValidElement(node) || node.type !== Row) return false;
  const props = node.props as RowProps;
  return !props.children && typeof props.desc === 'string' && props.desc.length >= NOTE_FROM;
}

/** A group of rows on one pane. The rows at its end that only explain come out of it as a line under it that opens the words
 *  on a click, unless they are all it has. */
export function Card({ children, className }: { children?: ReactNode; className?: string }) {
  const items = Children.toArray(children);
  const notes: ReactElement<RowProps>[] = [];
  if (!items.every(explains)) while (items.length > 1 && explains(items[items.length - 1])) notes.unshift(items.pop() as ReactElement<RowProps>);
  return (
    <>
      {items.length ? <section className={cx('card', className)}>{items}</section> : null}
      {notes.map((note) => (
        <Row key={note.key} {...note.props} note />
      ))}
    </>
  );
}

/** A state said in a word and a dot: orange for what is still to do, green once it is done. */
export function Pill({ children, good, tone }: { children: ReactNode; good?: boolean; tone?: 'good' | 'warn' | 'bad' | 'off' }) {
  return (
    <span className="pill" data-tone={tone ?? (good ? 'good' : 'warn')}>
      <span className="dot" />
      <span>{children}</span>
    </span>
  );
}
