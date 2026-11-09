import { useState } from 'react';
import { Button, IconButton, SelectRow, ToggleRow } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './weather.css';

interface Place {
  label: string;
}
interface WeatherState {
  enabled: boolean;
  found: Place[];
  places: Place[];
  full: boolean;
  searching: boolean;
  problem?: string;
}

/** Weather: a tab of the notch with the weather now, the next hours and the next days of the chosen cities, one at a time. */
export default function Weather() {
  const t = useT();
  const w = useSection<WeatherState>('weather');
  const on = w.enabled;
  const [query, setQuery] = useState('');
  const [asked, setAsked] = useState(false);
  const search = () => {
    const text = query.trim();
    if (text.length < 2) return;
    setAsked(true);
    send({ t: 'action', name: `weather_search:${text}` });
  };
  const tool = (verb: string, i: number, glyph: string, label: string, disabled: boolean, who: string) => (
    <IconButton size="sm" variant="glass" icon={glyph} disabled={disabled} label={`${label}: ${who}`} onClick={() => send({ t: 'action', name: `weather_${verb}:${i}` })} />
  );
  const status = w.searching ? t('weather.searching') : w.problem ? t(`weather.problem.${w.problem}`) : asked && !w.found.length ? t('weather.none') : '';
  return (
    <>
      <SectionTitle info={t('weather.places.desc')}>{t('sec.weather.places')}</SectionTitle>
      <Card>
        <Row title={null} block when={on}>
          <div className="weather-places">
            {w.places.length ? (
              w.places.map((p, i) => (
                <div className="weather-place" key={`${i}:${p.label}`}>
                  <span className="weather-number">{i + 1}</span>
                  <span className="weather-label">{p.label}</span>
                  <span className="weather-tools">
                    {tool('up', i, 'chevron.up', t('weather.up'), i === 0, p.label)}
                    {tool('remove', i, 'xmark', t('weather.remove'), false, p.label)}
                  </span>
                </div>
              ))
            ) : (
              <div className="row-desc">{t('weather.places.none')}</div>
            )}
          </div>
        </Row>
      </Card>

      <SectionTitle>{t('sec.weather.city')}</SectionTitle>
      <Card>
        <Row title={t('weather.city.title')} desc={t('weather.city.desc')} block when={on}>
          <div className="weather-search">
            <input
              type="text"
              className="text-field"
              aria-label={t('weather.city.label')}
              placeholder={t('weather.city.ph')}
              maxLength={60}
              spellCheck={false}
              autoComplete="off"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.nativeEvent.isComposing) search();
              }}
            />
            <Button small onClick={search}>
              {t('weather.search')}
            </Button>
          </div>
        </Row>
        <Row title={null} block when={on}>
          <div className="weather-found">
            <div className="weather-results">
              {w.found.map((p, i) => (
                <div className="weather-result" key={`${i}:${p.label}`}>
                  <span className="weather-label">{p.label}</span>
                  <Button small disabled={w.full} aria-label={`${t('weather.add')}: ${p.label}`} onClick={() => send({ t: 'action', name: `weather_add:${i}` })}>
                    {t('weather.add')}
                  </Button>
                </div>
              ))}
            </div>
            <div className="row-desc weather-status">{status}</div>
          </div>
        </Row>
      </Card>

      <SectionTitle>{t('sec.weather.units')}</SectionTitle>
      <Card>
        <SelectRow path="weather.units" title={t('weather.units.title')} desc={t('weather.units.desc')} when={on} options={['metric', 'imperial'].map((u) => ({ value: u, label: t(`weather.units.${u}`) }))} />
      </Card>

      <SectionTitle>{t('sec.weather.sound')}</SectionTitle>
      <Card>
        <ToggleRow path="weather.sound" title={t('weather.sound.title')} desc={t('weather.sound.desc')} when={on} />
      </Card>
    </>
  );
}
