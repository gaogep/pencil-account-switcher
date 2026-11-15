/** Development-only component catalogue; full pages are assembled in T3.3. */
import { Settings } from 'lucide-preact';
import type { ViewState } from './generated/ViewState';
import { AddAccountButton, AuthBadge, Avatar, BrandHeader, CurrentAccountCard, Hairline, IconButton, InkSwitch, LinkButton, NavHeader, OtherAccountRow, PillButton, QuotaProgressBar, RefreshButton, ReimportButton, StatusBanner, UpdateBanner, operationLocks } from './components';
export function ComponentGallery({ state }: { state: ViewState }) {
  const locks = operationLocks(state);
  const current = state.accounts.find(a => a.isCurrent);
  const others = state.accounts.filter(a => !a.isCurrent);
  return <>
    <BrandHeader actions={<><RefreshButton refreshing={state.refreshing} /><IconButton label="设置"><Settings size={15} strokeWidth={1.5} aria-hidden="true" /></IconButton></>} />
    {state.feedback && <StatusBanner feedback={state.feedback} />}
    <UpdateBanner update={state.update} />
    <div class="component-gallery">
      {current && <CurrentAccountCard account={current} state={state} />}
      {others.map(account => <OtherAccountRow key={account.id} account={account} state={state} />)}
      <AddAccountButton adding={state.addingAccount} disabled={state.addingAccount ? locks.cancelLogin : locks.addAccount} />
      <div class="gallery-row"><AuthBadge state="ok" /><AuthBadge state="expired" /><AuthBadge state="unknown" /></div>
      <div class="gallery-row"><Avatar email="alex.chen@gmail.com" size={36} /><ReimportButton email="alex.chen@gmail.com" /><LinkButton>导入</LinkButton><PillButton variant="remove">移除</PillButton></div>
      <div class="gallery-row"><InkSwitch checked={false} label="开机自启关闭" onChange={() => {}} /><InkSwitch checked label="开机自启开启" onChange={() => {}} /><RefreshButton refreshing /><RefreshButton refreshing={false} /></div>
      <QuotaProgressBar used={2} total={5} current /><QuotaProgressBar />
    </div>
    <Hairline />
    <NavHeader backLabel="账号列表" title="设置" subtitle="配置账号切换器" />
  </>;
}
