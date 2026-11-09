import { AddAppButton, AppList, RuleRow } from '@/components/macos/Apps';
import { Segmented, Select, SliderRow, Switch, ToggleRow } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { change, useApp } from '@/stores/app';
import { useSetting, useT } from '@/stores/hooks';
import { ScrollPad } from './ScrollPad';

// What the three feels of the wheel set: the step of one notch, the speed and how long a glide lasts.
const PRESETS = {
  precise: { step_px: 28, speed: 0.9, duration_ms: 220 },
  balanced: { step_px: 36, speed: 1, duration_ms: 360 },
  fluid: { step_px: 48, speed: 1.15, duration_ms: 560 },
} as const;

const BOOSTS = [2, 3, 5, 8, 12];
// The glyphs of shift, option and command.
const SHIFT = '\u21e7';
const OPTION = '\u2325';
const COMMAND = '\u2318';
const TIMES = '\u00d7';

/** The three feels of the wheel as one choice; moving a slider off all three leaves none chosen. */
function Presets() {
  const t = useT();
  const mouse = useApp((s) => s.state.mouse);
  const near = (a: number, b: number) => Math.abs(a - b) < 0.001;
  const names = Object.keys(PRESETS) as Array<keyof typeof PRESETS>;
  const now = names.find((name) => {
    const values = PRESETS[name];
    return near(mouse.step_px, values.step_px) && near(mouse.speed, values.speed) && near(mouse.duration_ms, values.duration_ms);
  });
  return (
    <Row title={t('sec.feel')}>
      <Segmented
        label={t('sec.feel')}
        value={now}
        options={names.map((name) => ({ value: name, label: t(`preset.${name}`) }))}
        onPick={(name) => {
          for (const [key, value] of Object.entries(PRESETS[name])) change(`mouse.${key}`, value);
        }}
      />
    </Row>
  );
}

function ModTitle({ symbol, text }: { symbol: string; text: string }) {
  return (
    <span>
      <kbd style={{ marginRight: 9 }}>{symbol}</kbd>
      {text}
    </span>
  );
}

export default function Scroll() {
  const t = useT();
  const boostNow = useSetting<number>('mouse.boost');
  const optionBoost = useSetting<boolean>('mouse.option_boost');
  const apps = useSetting<Array<{ id: string; name: string }>>('mouse.apps');
  const values = BOOSTS.includes(boostNow) ? BOOSTS : [...BOOSTS, boostNow];

  return (
    <>
      <SectionTitle>{t('sec.wheel')}</SectionTitle>
      <Card>
        <ToggleRow path="mouse.reverse" title={t('reverse.title')} desc={t('reverse.desc')} />
      </Card>

      <SectionTitle>{t('sec.feel')}</SectionTitle>
      <Card>
        <SliderRow path="mouse.step_px" title={t('step.title')} desc={t('step.desc')} min={8} max={120} step={1} format={(v) => `${Math.round(v)} px`} />
        <SliderRow path="mouse.speed" title={t('speed.title')} desc={t('speed.desc')} min={0.3} max={3} step={0.05} format={(v) => `${TIMES} ${v.toFixed(2)}`} />
        <SliderRow path="mouse.duration_ms" title={t('duration.title')} desc={t('duration.desc')} min={80} max={1000} step={10} format={(v) => `${Math.round(v)} ms`} />
        <Presets />
      </Card>

      <SectionTitle>{t('sec.try')}</SectionTitle>
      <ScrollPad />

      <SectionTitle>{t('sec.mods')}</SectionTitle>
      <Card>
        <ToggleRow path="mouse.shift_horizontal" title={<ModTitle symbol={SHIFT} text={t('shift.title')} />} label={t('shift.title')} desc={t('shift.desc')} />
        <Row title={<ModTitle symbol={OPTION} text={t('option.title')} />} desc={t('option.desc')}>
          <Select
            label={t('boost.title')}
            value={boostNow}
            disabled={!optionBoost}
            options={values.map((v) => ({ value: v, label: `${TIMES} ${v}` }))}
            onChange={(v) => change('mouse.boost', v)}
          />
          <Switch path="mouse.option_boost" label={t('option.title')} />
        </Row>
        <ToggleRow path="mouse.command_zoom" title={<ModTitle symbol={COMMAND} text={t('cmd.title')} />} label={t('cmd.title')} desc={t('cmd.desc')} />
      </Card>

      <SectionTitle>{t('sec.apps')}</SectionTitle>
      <Card>
        <Row title={t('apps.rules')} desc={t('apps.desc')}>
          <AddAppButton purpose="scroll" label={t('apps.add')} />
        </Row>
        <AppList apps={apps} empty={t('apps.empty')}>
          {(app) => <RuleRow rule={app} />}
        </AppList>
      </Card>
    </>
  );
}
