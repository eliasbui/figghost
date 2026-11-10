import { cn } from '@/lib/cn';
import { post } from './bridge';
import { ago } from './logic';
import type { State } from './logic';
import { Btn, Card, Grow, Header, Hint, Input, Mono, Notice, Row, Section, Toggle } from './ui';

interface Props {
  t: (key: string, vars?: Record<string, string | number>) => string;
  state: State;
}

const STATE_LOOK: Record<string, string> = {
  waiting: 'bg-warn text-white',
  working: 'bg-accent text-white',
  failed: 'bg-bad text-white',
  done: 'bg-good text-white',
};

export function AgentsPage({ t, state }: Props) {
  const { agents: a, config, env } = state;
  const cfg = config.agents;
  const when = (ms: number) => {
    const { key, n } = ago(ms, Date.now());
    return t(key, { n });
  };
  return (
    <>
      <Header title={t('page.agents')} sub={t('agents.sub')} />
      {env.os === 'windows' ? <Notice>{t('agents.windows')}</Notice> : null}
      {env.agents_error ? <Notice>{t('error', { e: env.agents_error })}</Notice> : null}
      <Card>
        <Row>
          <Grow>
            <div>{t('agents.enable')}</div>
            <Hint>{t('agents.enable.hint')}</Hint>
          </Grow>
          <Toggle checked={cfg.enabled} label={t('agents.enable')} onChange={(on) => post({ t: 'set', key: 'agents.enabled', value: on })} />
        </Row>
        <Row>
          <Grow>{t('agents.wait')}</Grow>
          <Input type="number" min={5} max={100} className="w-20" aria-label={t('agents.wait')} defaultValue={cfg.approval_wait_seconds} key={cfg.approval_wait_seconds} onChange={(event) => post({ t: 'set', key: 'agents.approval_wait_seconds', value: Number(event.target.value) })} />
        </Row>
      </Card>
      <Section>{t('agents.hooks')}</Section>
      <Card>
        {a.agents.map((agent) => (
          <Row key={agent.id}>
            <Grow>
              <div>{agent.name}</div>
              <Hint>{t(agent.installed ? 'agents.installed' : 'agents.missing')}</Hint>
            </Grow>
            <Btn onClick={() => post({ t: agent.installed ? 'agent_remove' : 'agent_install', agent: agent.id })}>{t(agent.installed ? 'agents.remove' : 'agents.install')}</Btn>
          </Row>
        ))}
      </Card>
      {a.approvals.length ? (
        <>
          <Section>{t('agents.waiting')}</Section>
          <Card>
            {a.approvals.map((q) => (
              <Row key={q.id}>
                <Grow>
                  <div>{q.question ? t('agents.asks', { agent: q.agent }) : t('agents.wants', { agent: q.agent, tool: q.tool })}</div>
                  <Mono>{q.question || q.detail}</Mono>
                  <Hint>{`${q.project}  -  ${when(q.requested)}`}</Hint>
                </Grow>
                {q.question ? (
                  <Btn onClick={() => post({ t: 'answer', id: q.id, allow: false })}>{t('agents.terminal')}</Btn>
                ) : (
                  <div className="flex gap-1.5">
                    <Btn variant="prominent" onClick={() => post({ t: 'answer', id: q.id, allow: true })}>
                      {t('agents.allow')}
                    </Btn>
                    <Btn variant="danger" onClick={() => post({ t: 'answer', id: q.id, allow: false })}>
                      {t('agents.deny')}
                    </Btn>
                  </div>
                )}
              </Row>
            ))}
          </Card>
        </>
      ) : null}
      <Section>{t('agents.sessions')}</Section>
      {a.sessions.length ? (
        <Card>
          {a.sessions.map((s, i) => (
            <Row key={i}>
              <Grow>
                <div>{`${s.agent}  -  ${s.project}`}</div>
                <Hint>{s.note || when(s.updated)}</Hint>
              </Grow>
              <span className={cn('rounded-full bg-[var(--fill)] px-2 py-px text-[11.5px]', STATE_LOOK[s.state])}>{t(`state.${s.state}`)}</span>
            </Row>
          ))}
        </Card>
      ) : (
        <div className="p-[30px] text-center text-fg-2">{t('agents.none')}</div>
      )}
    </>
  );
}
