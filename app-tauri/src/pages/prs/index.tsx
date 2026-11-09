import { ActionIcon, SelectRow, Switch, ToggleRow } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { useSection, useT } from '@/stores/hooks';
import './prs.css';

interface PrsState {
  enabled: boolean;
  github_tool: string;
  gitlab_tool: string;
  checking: boolean;
  count: number;
}

/** Pull requests: a tab of the notch with the review requests that wait for you on GitHub and GitLab. */
export default function Prs() {
  const t = useT();
  const p = useSection<PrsState>('prs');
  const on = p.enabled;
  return (
    <>
      <SectionTitle>{t('sec.prs.where')}</SectionTitle>
      <Card>
        <Row title={t('prs.github.title')} desc={t('prs.github.desc')} when={on}>
          <span className="prs-tool" data-state={p.github_tool}>
            {t(`prs.tool.${p.github_tool}`)}
          </span>
          <Switch path="prs.github" label={t('prs.github.title')} />
        </Row>
        <Row title={t('prs.gitlab.title')} desc={t('prs.gitlab.desc')} when={on}>
          <span className="prs-tool" data-state={p.gitlab_tool}>
            {t(`prs.tool.${p.gitlab_tool}`)}
          </span>
          <Switch path="prs.gitlab" label={t('prs.gitlab.title')} />
        </Row>
        <Row title={t('prs.check')} when={on}>
          <ActionIcon name="prs_check" icon="refresh" loading={p.checking} label={p.checking ? t('prs.checking') : t('prs.check')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.prs.list')}</SectionTitle>
      <Card>
        <ToggleRow path="prs.drafts" title={t('prs.drafts.title')} desc={t('prs.drafts.desc')} when={on} />
        <SelectRow path="prs.minutes" title={t('prs.minutes.title')} desc={t('prs.minutes.desc')} when={on} options={[5, 10, 30, 60].map((n) => ({ value: n, label: t('prs.minutes.n', { n }) }))} />
        <Row title={null} block when={on}>
          <div className="row-desc">{t('prs.count', { n: p.count })}</div>
        </Row>
      </Card>
    </>
  );
}
