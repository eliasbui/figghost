import { useEffect, useRef, useState } from 'react';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { change } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';

/** Video (beta): a tab of the notch that plays one stream or video file, muted. */
export default function Video() {
  const t = useT();
  const v = useSection<{ enabled: boolean; url: string }>('video');
  const [text, setText] = useState(v.url);
  const typing = useRef(false);
  // A field that is being typed in is not overwritten by the state that comes every two seconds.
  useEffect(() => {
    if (!typing.current) setText(v.url);
  }, [v.url]);
  const save = () => change('video.url', text.trim());
  return (
    <>
      <SectionTitle>{t('sec.video.source')}</SectionTitle>
      <Card>
        <Row title={t('video.url.title')} desc={t('video.url.desc')} block when={v.enabled}>
          <input
            type="text"
            className="text-field"
            aria-label={t('video.url.title')}
            placeholder={t('video.url.ph')}
            maxLength={2048}
            spellCheck={false}
            autoComplete="off"
            autoCapitalize="off"
            value={text}
            onFocus={() => {
              typing.current = true;
            }}
            onChange={(e) => setText(e.target.value)}
            onBlur={() => {
              typing.current = false;
              save();
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.nativeEvent.isComposing) save();
            }}
          />
        </Row>
      </Card>
    </>
  );
}
