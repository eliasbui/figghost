import type { ReactNode } from 'react';
import { Button as AriaButton, Menu, MenuItem, MenuTrigger, Popover, Separator } from 'react-aria-components';
import type { Placement } from 'react-aria-components';
import { cn } from '@/lib/cn';
import { button } from './Button';
import type { ButtonProps } from './Button';
import { Icon } from './Icon';

// -- a button that opens a short list

export interface MenuEntry {
  id: string;
  label: ReactNode;
  icon?: string;
  danger?: boolean;
  disabled?: boolean;
  onAction?: () => void;
  /** A line above this entry. */
  divider?: boolean;
}

const MENU = cn(
  'glass-pop min-w-[168px] rounded-[var(--r-pop)] p-1.5 text-[13px] outline-none',
  'data-[entering]:animate-[pop-in_180ms_var(--ease-out)] data-[exiting]:animate-[pop-out_110ms_ease-in_forwards]',
);

const ITEM = cn(
  'flex h-[30px] cursor-default items-center gap-2 rounded-[9px] px-2.5 outline-none select-none',
  'data-[focused]:bg-accent data-[focused]:text-white data-[disabled]:opacity-40 data-[danger]:text-bad data-[focused]:data-[danger]:text-white',
);

/** A button that opens a menu: it looks like any other button of the kit, and behaves as a menu button of the system does. */
export function MenuButton({
  label, icon, iconEnd, variant, size, entries, placement = 'bottom end',
}: { label: string; icon?: string; iconEnd?: string; variant?: ButtonProps['variant']; size?: ButtonProps['size']; entries: MenuEntry[]; placement?: Placement }) {
  return (
    <MenuTrigger>
      <AriaButton className={button({ variant, size })}>
        {icon ? <Icon name={icon} /> : null}
        {label}
        {iconEnd ? <Icon name={iconEnd} /> : null}
      </AriaButton>
      <Popover placement={placement} offset={6} className={MENU}>
        <Menu className="outline-none" onAction={(key) => entries.find((e) => e.id === key)?.onAction?.()}>
          {entries.flatMap((e) => [
            e.divider ? <Separator key={`${e.id}-line`} className="mx-2 my-1.5 h-px bg-hair" /> : null,
            <MenuItem key={e.id} id={e.id} textValue={typeof e.label === 'string' ? e.label : e.id} isDisabled={e.disabled} data-danger={e.danger ? '' : undefined} className={ITEM}>
              {e.icon ? <Icon name={e.icon} className="text-[16px] opacity-80" /> : null}
              <span className="truncate">{e.label}</span>
            </MenuItem>,
          ])}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}
