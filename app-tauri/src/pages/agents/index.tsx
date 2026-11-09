import { Button, Select, ToggleRow } from '@/components/macos/Controls';
import { Advanced, confirmDialog } from '@/components/macos/Dialogs';
import { Card, InfoRow, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { compact, cx, usd } from '@/lib/text';
import { change, send } from '@/stores/app';
import { useLang, useSection, useT } from '@/stores/hooks';

interface AgentStatus {
  found: boolean;
  installed: number;
  wanted: number;
  error: string | null;
}
interface Limit {
  label: string;
  fraction: number | null;
  value: string;
  reset: string;
}
interface Account {
  agent: 'claude' | 'codex';
  id: string;
  email: string;
  name: string;
  plan: string;
  orca: boolean;
  cli: boolean;
  note: string;
  limits: Limit[];
}
interface Accounts {
  list: Account[];
  cli?: string;
  restore: Record<string, boolean>;
  busy?: boolean;
  updated?: string;
  outcome?: { ok: boolean; what: string; detail: string };
  keychain: boolean;
}
interface Spend {
  agent: string;
  today: number;
  week: number;
  today_cost: number | null;
  week_cost: number | null;
}
interface AgentsState {
  enabled: boolean;
  wait: number;
  tokens: boolean;
  usage_tap: boolean;
  usage_known: boolean;
  spend?: Spend[];
  accounts?: Accounts;
  claude: AgentStatus;
  codex: AgentStatus;
  pi: AgentStatus;
  agy: AgentStatus;
}

const AGENTS = ['claude', 'codex', 'pi', 'agy'] as const;
const WAITS = [10, 20, 30, 60, 100];
const DOT = '\u00b7';

/** One coding agent: whether its hooks are in its file, and the button that puts them there or takes them out. */
function AgentLine({ which, a }: { which: (typeof AGENTS)[number]; a: AgentStatus }) {
  const t = useT();
  const complete = a.installed > 0 && a.installed === a.wanted && !a.error;
  const status = a.error
    ? t('agents.status.error')
    : !a.found && a.installed === 0
      ? t('agents.status.missing')
      : a.installed === 0
        ? t('agents.status.off')
        : a.installed < a.wanted
          ? t('agents.status.partial', { n: a.installed, m: a.wanted })
          : t('agents.status.on');
  const label = complete ? t('agents.remove') : a.installed > 0 ? t('agents.repair') : t('agents.install');
  return (
    <Row title={t(`agents.${which}.title`)} desc={t(`agents.${which}.desc`)}>
      <Pill good={complete}>{status}</Pill>
      <Button
        small
        danger={complete}
        disabled={!a.found && a.installed === 0}
        aria-label={`${label} ${t(`agents.${which}.title`)}`}
        onClick={() => send({ t: 'action', name: `agents_${complete ? 'remove_' : 'install_'}${which}` })}
      >
        {label}
      </Button>
    </Row>
  );
}

function UsageRow({ agents }: { agents: AgentsState }) {
  const t = useT();
  const label = agents.usage_tap ? t('agents.usage.off') : t('agents.usage.on');
  return (
    <Row
      title={t('agents.usage.title')}
      desc={t('agents.usage.desc')}
      below={
        agents.usage_tap && !agents.usage_known ? (
          <div className="row-desc" role="status">
            {t('agents.usage.waiting')}
          </div>
        ) : null
      }
    >
      <Pill good={agents.usage_tap}>{agents.usage_tap ? t('agents.usage.ok') : t('agents.usage.none')}</Pill>
      <Button
        small
        disabled={!agents.claude.found && !agents.usage_tap}
        aria-label={`${label} ${t('agents.usage.title')}`}
        onClick={() => send({ t: 'action', name: agents.usage_tap ? 'agents_usage_off' : 'agents_usage_on' })}
      >
        {label}
      </Button>
    </Row>
  );
}

// -- accounts: the Claude Code and Codex accounts Orca keeps, their limits, and switching the account of the plain claude
// and codex commands (see crates/app-ui/src/accounts.rs)

/** One window of an account's limits, drawn like the bars of the notch. */
function AccountLimit({ limit }: { limit: Limit }) {
  const used = limit.fraction == null ? 0 : Math.min(1, Math.max(0, limit.fraction));
  const tone = used >= 0.9 ? 'bad' : used >= 0.7 ? 'warn' : '';
  return (
    <div className="acct-limit">
      <span>{limit.label}</span>
      <span className={cx('acct-bar', tone)} role="img" aria-label={`${limit.label} ${limit.value}`}>
        <i style={{ width: `${used * 100}%` }} />
      </span>
      <span className="acct-value">{limit.value}</span>
      <span className="acct-reset">{limit.reset}</span>
    </div>
  );
}

function AccountRow({ account, accounts }: { account: Account; accounts: Accounts }) {
  const t = useT();
  const details = [account.name !== account.email && account.name, account.plan, account.agent === 'claude' ? 'Claude Code' : 'Codex'];
  const note = account.agent === 'codex' ? 'codex' : account.note || (account.limits.length ? '' : 'none');
  return (
    <Row
      title={account.email}
      below={
        <>
          <div>{details.filter(Boolean).join(` ${DOT} `)}</div>
          {account.limits.map((limit) => (
            <AccountLimit key={limit.label} limit={limit} />
          ))}
          {note ? <div className="row-desc">{t(`accounts.note.${note}`)}</div> : null}
        </>
      }
    >
      {account.cli ? <Pill good>{t(`accounts.badge.cli.${account.agent}`)}</Pill> : null}
      {account.orca ? <Pill good>{t('accounts.badge.orca')}</Pill> : null}
      {account.cli ? null : (
        <Button
          small
          disabled={!!accounts.busy || (account.agent === 'claude' && !accounts.keychain)}
          aria-label={`${t('accounts.switch')}: ${account.email}`}
          onClick={() =>
            confirmDialog(
              { title: t('accounts.switch.title', { email: account.email, cmd: account.agent }), body: t(`accounts.switch.body.${account.agent}`), confirm: t('accounts.switch') },
              () => send({ t: 'action', name: `accounts_switch:${account.id}` }),
            )
          }
        >
          {t('accounts.switch')}
        </Button>
      )}
    </Row>
  );
}

function RestoreRow({ agent, accounts }: { agent: 'claude' | 'codex'; accounts: Accounts }) {
  const t = useT();
  if (!accounts.restore[agent]) return null;
  return (
    <Row title={t(`accounts.restore.${agent}`)} desc={t('accounts.restore.desc')}>
      <Button
        small
        disabled={!!accounts.busy}
        aria-label={t(`accounts.restore.${agent}`)}
        onClick={() =>
          confirmDialog({ title: t('accounts.restore.confirm'), body: t('accounts.restore.desc'), confirm: t('accounts.restore') }, () =>
            send({ t: 'action', name: `accounts_restore:${agent}` }),
          )
        }
      >
        {t('accounts.restore')}
      </Button>
    </Row>
  );
}

function AccountsCard({ accounts }: { accounts: Accounts }) {
  const t = useT();
  const inOrca = accounts.list.some((a) => a.agent === 'claude' && a.cli);
  const said = [
    accounts.cli && t(inOrca ? 'accounts.cli' : 'accounts.cli.outside', { email: accounts.cli }),
    accounts.outcome && t(`accounts.done.${accounts.outcome.what}`, { detail: accounts.outcome.detail }),
  ]
    .filter(Boolean)
    .join(' ');
  const status = accounts.busy ? t('accounts.asking') : !accounts.keychain ? t('accounts.nokeychain') : accounts.updated ? t('accounts.updated', { when: accounts.updated }) : '';
  return (
    <Card>
      <InfoRow
        title={t('accounts.intro.title')}
        desc={t('accounts.intro.body')}
        below={
          said ? (
            <div className="row-desc" role="status">
              {said}
            </div>
          ) : null
        }
      />
      <div className="apps">
        {accounts.list.length ? accounts.list.map((account) => <AccountRow key={account.id} account={account} accounts={accounts} />) : <div className="empty">{t('accounts.empty')}</div>}
      </div>
      <Row
        title={t('accounts.refresh.title')}
        desc={t('accounts.limits.desc')}
        below={
          status ? (
            <div className="row-desc" role="status">
              {status}
            </div>
          ) : null
        }
      >
        <Button small disabled={!!accounts.busy || !accounts.keychain} onClick={() => send({ t: 'action', name: 'accounts_refresh' })}>
          {t('accounts.refresh')}
        </Button>
      </Row>
      <RestoreRow agent="claude" accounts={accounts} />
      <RestoreRow agent="codex" accounts={accounts} />
    </Card>
  );
}

/** What one agent used today and in the week, with the cost when there is a price. */
function SpendRow({ spend }: { spend: Spend }) {
  const t = useT();
  const lang = useLang();
  const line = (key: string, tokens: number, cost: number | null) => t(key, { tokens: compact(tokens, lang) }) + (cost == null ? '' : ` ${DOT} ${usd(cost, lang)}`);
  return (
    <Row title={t(`agents.spend.${spend.agent}`)} below={line('agents.spend.today', spend.today, spend.today_cost)}>
      <span className="row-title">{line('agents.spend.week', spend.week, spend.week_cost)}</span>
    </Row>
  );
}

export default function Agents() {
  const t = useT();
  const agents = useSection<AgentsState>('agents');
  const accounts: Accounts = agents.accounts ?? { list: [], restore: {}, keychain: false };
  const waits = WAITS.includes(agents.wait) ? WAITS : [...WAITS, agents.wait];
  const spend = agents.tokens ? (agents.spend ?? []) : [];

  return (
    <>
      <SectionTitle>{t('sec.agents.connect')}</SectionTitle>
      <Card>
        {AGENTS.map((which) => (
          <AgentLine key={which} which={which} a={agents[which]} />
        ))}
      </Card>

      <SectionTitle>{t('sec.agents.answer')}</SectionTitle>
      <Card>
        <Row title={t('agents.wait.title')} desc={t('agents.wait.desc')} when={agents.enabled}>
          <Select label={t('agents.wait.title')} value={agents.wait} options={waits.map((n) => ({ value: n, label: t('agents.wait.n', { n }) }))} onChange={(v) => change('agents.wait', v)} />
        </Row>
        <ToggleRow path="agents.open_on_ask" title={t('agents.ask.title')} desc={t('agents.ask.desc')} />
        <ToggleRow path="agents.sound" title={t('agents.sound.title')} desc={t('agents.sound.desc')} />
        <ToggleRow path="agents.peek" title={t('agents.peek.title')} desc={t('agents.peek.desc')} />
      </Card>

      <SectionTitle>{t('sec.agents.limits')}</SectionTitle>
      <Card>
        <UsageRow agents={agents} />
      </Card>

      <SectionTitle>{t('sec.agents.note')}</SectionTitle>
      <Card>
        <InfoRow title={t('agents.note.title')} desc={t('agents.note.body')} />
      </Card>

      <Advanced>
        <SectionTitle>{t('sec.agents.accounts')}</SectionTitle>
        <AccountsCard accounts={accounts} />
        <SectionTitle>{t('sec.agents.tokens')}</SectionTitle>
        <Card>
          <ToggleRow path="agents.tokens" title={t('agents.tokens.title')} desc={t('agents.tokens.desc')} />
          <div className="apps">{spend.length ? spend.map((s) => <SpendRow key={s.agent} spend={s} />) : <div className="empty">{t('agents.spend.none')}</div>}</div>
        </Card>
      </Advanced>
    </>
  );
}
