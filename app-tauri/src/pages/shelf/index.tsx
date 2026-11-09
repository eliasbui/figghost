import { Button } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useSetting, useT } from '@/stores/hooks';

/** The shelf, a tab of the notch: what is on it, and a button to empty it. */
export default function Shelf() {
  const t = useT();
  const count = useSetting<number>('notch.shelf_count');
  return (
    <>
      <SectionTitle>{t('sec.shelf')}</SectionTitle>
      <Card>
        <Row title={t('shelf.count')}>
          <span className="row-title">{count}</span>
        </Row>
        <Row title={t('shelf.clear')} desc={t('shelf.clear.desc')}>
          <Button small icon="trash" onClick={() => send({ t: 'action', name: 'clear_shelf' })}>
            {t('shelf.clear')}
          </Button>
        </Row>
      </Card>
    </>
  );
}
