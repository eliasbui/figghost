import { SelectRow, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { useSection, useT } from '@/stores/hooks';

/** Notes: a scratchpad in a tab of the notch, one plain text file for each note. */
export default function Notes() {
  const t = useT();
  const notes = useSection<{ enabled: boolean; count: number }>('notes');
  const on = notes.enabled;
  return (
    <>
      <SectionTitle>{t('sec.notes.writing')}</SectionTitle>
      <Card>
        <SelectRow path="notes.text_size" title={t('notes.size.title')} desc={t('notes.size.desc')} when={on} options={[12, 14, 16].map((n) => ({ value: n, label: t(`notes.size.${n}`) }))} />
        <ToggleRow path="notes.new_on_open" title={t('notes.new.title')} desc={t('notes.new.desc')} when={on} />
      </Card>

      <SectionTitle>{t('sec.notes.files')}</SectionTitle>
      <Card>
        <Row title={t('notes.folder.title')} desc={t('notes.folder.desc', { count: notes.count })} below={t('ui.notes', { count: notes.count })} when={on}>
          <ActionIcon name="notes_folder" icon="folder" label={t('notes.folder.open')} />
        </Row>
      </Card>
    </>
  );
}
