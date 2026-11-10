import { OS_NAMES } from './logic';
import type { State } from './logic';
import { Card, Grow, Header, Notice, Row, Section } from './ui';

interface Props {
  t: (key: string, vars?: Record<string, string | number>) => string;
  state: State;
}

function List({ text }: { text: string }) {
  return (
    <ul className="ml-5 list-disc space-y-1">
      {text.split('|').map((line) => (
        <li key={line}>{line}</li>
      ))}
    </ul>
  );
}

export function AboutPage({ t, state }: Props) {
  const { env } = state;
  const os = OS_NAMES[env.os] ?? env.os;
  return (
    <>
      <Header title={t('page.about')} sub={t('about.sub')} />
      <Notice>
        <strong>{t('beta')}. </strong>
        {t('foot', { os })}
      </Notice>
      {env.wayland ? <Notice>{t('about.wayland')}</Notice> : null}
      <Card>
        <Row>
          <Grow>{t('about.version')}</Grow>
          <span>{`${env.version} beta, ${os}`}</span>
        </Row>
      </Card>
      <Section>{t('about.here')}</Section>
      <List text={t('about.here.list')} />
      <Section>{t('about.later')}</Section>
      <List text={t('about.later.list')} />
      <Section>{t('about.limits')}</Section>
      <List text={t('about.limits.list')} />
    </>
  );
}
