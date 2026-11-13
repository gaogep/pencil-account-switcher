import { CircleCheck } from 'lucide-preact';
import type { AccountRecord } from '../generated/AccountRecord';
import type { ViewState } from '../generated/ViewState';
import { AuthBadge, Avatar, PillButton, QuotaProgressBar, ReimportButton, authLabels, operationLocks, quotaText } from './primitives';
type Props = { account: AccountRecord; state: ViewState; onReimport?: () => void };
export function CurrentAccountCard({ account, state, onReimport }: Props) {
  const locks = operationLocks(state);
  return <section class="current-account" aria-label={`当前账号 ${account.email}，${authLabels[account.authState]}`}><div class="current-top"><span>当前使用</span><AuthBadge state={account.authState} /></div><div class="account-info"><Avatar email={account.email} size={36} /><div class="account-identity"><strong title={account.email}>{account.email}</strong>{account.plan !== undefined && <span>{account.plan}</span>}</div>{account.authState !== 'ok' && <ReimportButton email={account.email} onClick={onReimport} disabled={locks.reimport} />}<CircleCheck size={16} strokeWidth={1.5} class="current-check" aria-hidden="true" /></div><div class="current-quota"><div class="quota-labels"><span>MCP 使用天数</span><strong title={account.lastError}>{quotaText(account.usedDays, account.totalDays)}</strong></div><QuotaProgressBar current used={account.usedDays} total={account.totalDays} /></div></section>;
}
export function OtherAccountRow({ account, state, onReimport, onSwitch }: Props & { onSwitch?: () => void }) {
  const locks = operationLocks(state);
  const switching = state.switchingTo?.toLowerCase() === account.id.toLowerCase();
  return <section class="other-account"><div class="other-top"><Avatar email={account.email} /><div class="account-identity"><strong title={account.email}>{account.email}</strong>{account.plan !== undefined && <span>{account.plan}</span>}</div><div class="account-actions"><AuthBadge state={account.authState} />{account.authState !== 'ok' && <ReimportButton email={account.email} onClick={onReimport} disabled={locks.reimport} />}<PillButton onClick={onSwitch} disabled={locks.switchAccount} label={`切换到 ${account.email}`} title={account.authState === 'expired' ? '登录已失效，切换后若仍无法使用请点重新导入' : undefined}>{switching ? '切换中…' : '切换'}</PillButton></div></div><div class="other-quota"><span>MCP 使用天数</span><strong title={account.lastError}>{quotaText(account.usedDays, account.totalDays)}</strong><QuotaProgressBar used={account.usedDays} total={account.totalDays} width={140} /></div></section>;
}
