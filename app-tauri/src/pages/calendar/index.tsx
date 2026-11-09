import { Button, Select, ToggleRow } from '@/components/macos/Controls';
import { Card, InfoRow, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { cx } from '@/lib/text';
import { change, patch, send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './calendar.css';

const LEADS = [1, 2, 5, 10, 15, 30, 60];
const KNOWN_STATES = ['granted', 'not_determined', 'denied', 'restricted', 'write_only'];

interface Item {
  id: string;
  title: string;
  color: string;
  source?: string;
  included: boolean;
}
interface CalendarState {
  enabled: boolean;
  lead_minutes: number;
  can_prompt: boolean;
  events_access: string;
  reminders_access: string;
  calendars: Item[];
  lists: Item[];
}

/** The access to events or reminders: what the system says about it, and the button that asks or opens the place to change it. */
function AccessRow({ kind, cal }: { kind: 'events' | 'reminders'; cal: CalendarState }) {
  const t = useT();
  const state = kind === 'events' ? cal.events_access : cal.reminders_access;
  const ask = state === 'not_determined' && cal.can_prompt;
  const open = state === 'denied' || state === 'write_only';
  const label = ask ? t('cal.allow') : t('cal.open');
  return (
    <Row
      title={t(`cal.access.${kind}`)}
      desc={t(`cal.access.${kind}.desc`)}
      below={
        state === 'not_determined' && !cal.can_prompt ? (
          <div className="row-desc" role="status">
            {t('cal.noprompt')}
          </div>
        ) : null
      }
    >
      <Pill good={state === 'granted'}>{t(`cal.state.${KNOWN_STATES.includes(state) ? state : 'unknown'}`)}</Pill>
      {ask || open ? (
        <Button small aria-label={`${label}: ${t(`cal.access.${kind}`)}`} onClick={() => send({ t: 'action', name: `calendar_access_${kind}` })}>
          {label}
        </Button>
      ) : null}
    </Row>
  );
}

/** One calendar or list: a box to include it, its colour, its name and where it comes from. */
function ItemRow({ item, cal }: { item: Item; cal: CalendarState }) {
  const t = useT();
  const name = item.title || item.id;
  return (
    <div className={cx('cal-row', !item.included && 'off')}>
      <button
        type="button"
        className="cal-check"
        role="checkbox"
        aria-checked={item.included}
        aria-label={t('cal.include', { name })}
        onClick={() => {
          const next = !item.included;
          for (const key of ['calendars', 'lists'] as const) {
            patch(`calendar.${key}`, cal[key].map((c) => (c.id === item.id ? { ...c, included: next } : c)));
          }
          send({ t: 'set', path: 'calendar.include', value: { id: item.id, on: next } });
        }}
      />
      <span className="cal-swatch" style={{ background: /^#[0-9a-f]{6}$/i.test(item.color) ? item.color : '#8e8e93' }} aria-hidden="true" />
      <span className="cal-name">{name}</span>
      {item.source ? <span className="cal-source">{item.source}</span> : null}
    </div>
  );
}

function ItemList({ field, cal, empty }: { field: 'calendars' | 'lists'; cal: CalendarState; empty: string }) {
  const t = useT();
  const items = cal[field];
  return <div className="cal-list">{items.length ? items.map((item) => <ItemRow key={item.id} item={item} cal={cal} />) : <div className="empty">{t(empty)}</div>}</div>;
}

/** Calendar, reminders and meeting alerts on the notch: the settings page. */
export default function Calendar() {
  const t = useT();
  const cal = useSection<CalendarState>('calendar');
  const on = cal.enabled;
  const leads = LEADS.includes(cal.lead_minutes) ? LEADS : [...LEADS, cal.lead_minutes];
  return (
    <>
      <SectionTitle>{t('cal.sec.alerts')}</SectionTitle>
      <Card>
        <Row title={t('cal.lead.title')} desc={t('cal.lead.desc')} when={on}>
          <Select
            label={t('cal.lead.title')}
            value={cal.lead_minutes}
            options={leads.map((n) => ({ value: n, label: t('cal.lead.n', { n }) }))}
            onChange={(v) => change('calendar.lead_minutes', v)}
          />
        </Row>
        <ToggleRow path="calendar.sound" title={t('cal.sound.title')} desc={t('cal.sound.desc')} when={on} />
      </Card>

      <SectionTitle>{t('cal.sec.show')}</SectionTitle>
      <Card>
        <ToggleRow path="calendar.reminders" title={t('cal.reminders.title')} desc={t('cal.reminders.desc')} when={on} />
        <ToggleRow path="calendar.all_day" title={t('cal.allday.title')} desc={t('cal.allday.desc')} when={on} />
      </Card>

      <SectionTitle>{t('cal.sec.access')}</SectionTitle>
      <Card>
        <AccessRow kind="events" cal={cal} />
        <AccessRow kind="reminders" cal={cal} />
      </Card>

      <SectionTitle>{t('cal.sec.calendars')}</SectionTitle>
      <Card>
        <ItemList field="calendars" cal={cal} empty="cal.calendars.empty" />
      </Card>

      <SectionTitle>{t('cal.sec.lists')}</SectionTitle>
      <Card>
        <ItemList field="lists" cal={cal} empty="cal.lists.empty" />
      </Card>

      <SectionTitle>{t('cal.sec.note')}</SectionTitle>
      <Card>
        <InfoRow title={t('cal.note.title')} desc={t('cal.note.body')} />
      </Card>
    </>
  );
}
