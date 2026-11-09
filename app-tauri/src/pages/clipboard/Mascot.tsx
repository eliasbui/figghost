import { Button, Segmented, SegRow, ToggleRow } from '@/components/macos/Controls';
import { Card, InfoRow, Row, SectionTitle } from '@/components/macos/Layout';
import { change, send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';
import './mascot.css';

interface MascotState {
  character: string;
  characters: string[];
  sheets?: Record<string, string>;
  accessibility: boolean;
}

/** The clipboard mascot: a card at the end of the Clipboard page. */
export function MascotCards() {
  const t = useT();
  const mascot = useSection<MascotState>('mascot');
  const c = mascot.character;
  return (
    <>
      <SectionTitle>{t('mascot.sec')}</SectionTitle>
      <Card>
        <ToggleRow path="mascot.enabled" title={t('mascot.enable.title')} desc={t('mascot.enable.desc')} />
        <Row title={t('mascot.character.title')} desc={t('mascot.character.desc')} block>
          <div className="mascot-pick">
            <div className="mascot-sprite" role="img" aria-label={t(`mascot.character.${c}`)} style={{ backgroundImage: `url("${mascot.sheets?.[c] ?? ''}")` }} />
            <Segmented label={t('mascot.character.title')} value={c} options={mascot.characters.map((id) => ({ value: id, label: t(`mascot.character.${id}`) }))} onPick={(id) => change('mascot.character', id)} />
          </div>
        </Row>
        <SegRow path="mascot.farewell" title={t('mascot.farewell.title')} desc={t('mascot.farewell.desc')} values={['guard', 'regret']} />
        <Row title={t('mascot.try.title')} desc={t('mascot.try.desc')}>
          <Button small onClick={() => send({ t: 'action', name: 'mascot_try' })}>
            {t('mascot.try')}
          </Button>
        </Row>
        <Row title={t('mascot.aim.title')} block>
          <div className="row-desc">{t(mascot.accessibility ? 'mascot.aim.on' : 'mascot.aim.off')}</div>
        </Row>
        <InfoRow title={t('mascot.finder.title')} desc={t('mascot.finder.desc')} />
      </Card>
    </>
  );
}
