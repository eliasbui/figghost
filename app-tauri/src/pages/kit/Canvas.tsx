import { useState } from 'react';
import type { ReactNode } from 'react';
import { Button, ButtonGroup, IconButton, MenuButton, Segmented, Select, SureButton, ToggleButton } from '@/components/macos/Controls';
import { Slider, SwitchButton } from '@/components/macos/Controls';
import { Card, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { Keycaps } from '@/components/macos/Controls';
import { Icon } from '@/components/macos/Icon';
import { PageIcon } from '@/components/macos/Page';
import { PAGE_SYMBOLS } from '@/pages/meta';
import { showToast } from '@/stores/app';
import { ConfirmHost, confirmDialog } from '@/components/macos/Dialogs';

/** The kit on one page, for looking at it: every kind of button, every control and a few rows. Open it with ?canvas=kit. */
function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <>
      <SectionTitle>{title}</SectionTitle>
      <Card>
        <div className="flex flex-wrap items-center gap-3 p-4">{children}</div>
      </Card>
    </>
  );
}

export default function Kit() {
  const [on, setOn] = useState(true);
  const [seg, setSeg] = useState('b');
  const [choice, setChoice] = useState('one');
  const [level, setLevel] = useState(40);
  const [pressed, setPressed] = useState(false);
  return (
    <div id="app" style={{ overflow: 'auto' }}>
      <div className="mx-auto max-w-[760px] px-6 py-8">
        <h1 className="hero-title mb-2">Kit</h1>

        <Group title="Buttons">
          <Button>Glass</Button>
          <Button variant="prominent">Prominent</Button>
          <Button variant="tinted">Tinted</Button>
          <Button variant="outline">Outline</Button>
          <Button variant="plain">Plain</Button>
          <Button variant="danger">Danger</Button>
          <Button variant="destructive">Delete</Button>
          <Button variant="link">Link</Button>
          <Button disabled>Disabled</Button>
          <Button loading>Busy</Button>
        </Group>
        <Group title="Sizes and shapes">
          <Button size="xs">Extra small</Button>
          <Button size="sm">Small</Button>
          <Button size="md">Medium</Button>
          <Button size="lg" variant="prominent">Large</Button>
          <Button shape="rounded">Rounded</Button>
          <Button icon="plus">With picture</Button>
          <Button iconEnd="chevron.down">Menu</Button>
          <Button block variant="prominent">Block</Button>
        </Group>
        <Group title="Pictures only">
          <IconButton icon="plus" label="Add" variant="glass" />
          <IconButton icon="refresh" label="Refresh" />
          <IconButton icon="trash" label="Delete" variant="danger" />
          <IconButton icon="close" label="Close" size="sm" />
          <IconButton icon="search" label="Search" variant="prominent" size="lg" />
          <ButtonGroup label="Group">
            <IconButton icon="chevron.left" label="Back" />
            <IconButton icon="chevron.right" label="Forward" />
          </ButtonGroup>
          <ToggleButton pressed={pressed} onPressedChange={setPressed} icon="pin">
            Pin
          </ToggleButton>
          <MenuButton
            label="Actions"
            iconEnd="chevron.down"
            entries={[
              { id: 'a', label: 'Copy', icon: 'copy', onAction: () => showToast('Copied') },
              { id: 'b', label: 'Edit', icon: 'edit' },
              { id: 'c', label: 'Delete', icon: 'trash', danger: true, divider: true },
            ]}
          />
          <SureButton label="Remove" sure="Sure?" icon="trash" onConfirm={() => showToast('Removed')} />
        </Group>
        <Group title="Controls">
          <SwitchButton checked={on} onToggle={() => setOn(!on)} label="Switch" />
          <SwitchButton checked={false} onToggle={() => {}} label="Off" />
          <SwitchButton checked disabled onToggle={() => {}} label="Disabled" />
          <Segmented value={seg} onPick={setSeg} label="Seg" options={[{ value: 'a', label: 'Auto' }, { value: 'b', label: 'Light' }, { value: 'c', label: 'Dark' }]} />
          <Segmented small value={seg} onPick={setSeg} label="Small" options={[{ value: 'a', label: <Icon name="sun" />, name: 'Sun' }, { value: 'b', label: <Icon name="moon" />, name: 'Moon' }, { value: 'c', label: <Icon name="sliders" />, name: 'Sliders' }]} />
          <Select value={choice} onChange={(v) => setChoice(String(v))} label="Select" options={[{ value: 'one', label: 'One' }, { value: 'two', label: 'Two' }, { value: 'three', label: 'Three' }]} />
          <Select small value={choice} onChange={(v) => setChoice(String(v))} label="Small" options={[{ value: 'one', label: 'One' }, { value: 'two', label: 'Two' }]} />
          <Slider value={level} min={0} max={100} step={1} onChange={setLevel} label="Level" format={(v) => `${v} %`} />
          <input className="text-field" style={{ width: 200 }} placeholder="Field" aria-label="Field" />
          <Keycaps shortcut="cmd+shift+c" />
          <Pill good>Granted</Pill>
          <Pill>Missing</Pill>
          <Pill tone="bad">Failed</Pill>
          <Pill tone="off">Off</Pill>
        </Group>
        <Group title="Pages">
          {Object.keys(PAGE_SYMBOLS).slice(0, 18).map((id) => (
            <PageIcon key={id} id={id} weight="fill" className="hero-tile !size-9 !rounded-[11px] !text-[20px]" />
          ))}
        </Group>
        <SectionTitle>Rows</SectionTitle>
        <Card>
          <Row title="Open at login" desc="Start Figghost when you log in. The window stays closed; use the menu bar icon.">
            <SwitchButton checked={on} onToggle={() => setOn(!on)} label="x" />
          </Row>
          <Row title="Appearance">
            <Segmented value={seg} onPick={setSeg} label="Seg" options={[{ value: 'a', label: 'Auto' }, { value: 'b', label: 'Light' }, { value: 'c', label: 'Dark' }]} />
          </Row>
          <Row title="Speed" desc="How far one notch of the wheel goes.">
            <Slider value={level} min={0} max={100} step={1} onChange={setLevel} label="Speed" format={(v) => `${v} px`} />
          </Row>
          <Row title="Language">
            <Select value={choice} onChange={(v) => setChoice(String(v))} label="Language" options={[{ value: 'one', label: 'English' }, { value: 'two', label: 'Tieng Viet' }]} />
          </Row>
          <Row title="Accessibility" desc="Needed for smooth scrolling.">
            <Pill good>Granted</Pill>
            <IconButton icon="open" label="Open" variant="glass" />
          </Row>
          <Row title="Reset everything">
            <Button variant="danger" onClick={() => confirmDialog({ title: 'Reset?', body: 'This cannot be undone.', confirm: 'Reset' }, () => showToast('Done'))}>
              Reset
            </Button>
          </Row>
        </Card>
      </div>
      <ConfirmHost />
    </div>
  );
}
