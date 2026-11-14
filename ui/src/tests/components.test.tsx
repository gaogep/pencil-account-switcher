import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/preact';
import fixture from '../../public/scenarios/main.json';
import type { ViewState } from '../generated/ViewState';
import { AuthBadge, CurrentAccountCard, InkSwitch, OtherAccountRow, RefreshButton, StatusBanner, UpdateBanner, operationLocks, stableHash } from '../components';
import { MainScreen, ManageScreen, SettingsScreen } from '../screens';
import { ConfirmDialog } from '../ConfirmDialog';
const state = () => structuredClone(fixture.state) as ViewState;
const actions = () => ({ send: vi.fn(), navigate: vi.fn() });
describe('component state and actions', () => {
  it.each(['ok', 'expired', 'unknown'] as const)('renders authentication state %s', authState => {
    render(<AuthBadge state={authState} />); expect(screen.getByText({ ok: '生效中', expired: '已失效', unknown: '待确认' }[authState])).toBeVisible();
  });
  it('does not fabricate missing quota and preserves long email text', () => {
    const view = state(); const account = view.accounts[0]; account.usedDays = undefined; account.totalDays = undefined; account.email = 'very.long.account.name.with.extra.characters@example.test';
    render(<CurrentAccountCard account={account} state={view} />);
    expect(screen.getByText('— / — 天')).toBeVisible(); expect(screen.getByTitle(account.email)).toHaveTextContent(account.email);
    expect(screen.getByRole('progressbar')).not.toHaveAttribute('aria-valuenow');
  });
  it('prevents switching while refreshing', () => {
    const view = state(); view.refreshing = true; const click = vi.fn();
    render(<OtherAccountRow account={view.accounts[1]} state={view} onSwitch={click} />);
    const button = screen.getByRole('button', { name: `切换到 ${view.accounts[1].email}` }); button.click();
    expect(button).toBeDisabled(); expect(click).not.toHaveBeenCalled();
  });
  it('keeps switches controlled by the supplied state', () => {
    const change = vi.fn(); const { rerender } = render(<InkSwitch checked={false} label="开机自动启动" onChange={change} />);
    screen.getByRole('switch').click(); expect(change).toHaveBeenCalledWith(true);
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
    rerender(<InkSwitch checked label="开机自动启动" disabled onChange={change} />); screen.getByRole('switch').click();
    expect(change).toHaveBeenCalledTimes(1); expect(screen.getByRole('switch')).toBeDisabled();
  });
  it('keeps refresh icon mounted across state updates', () => {
    const { rerender } = render(<RefreshButton refreshing />); const icon = screen.getByRole('button').querySelector('svg');
    rerender(<RefreshButton refreshing />); expect(screen.getByRole('button').querySelector('svg')).toBe(icon);
    expect(screen.getByRole('button')).toBeDisabled(); expect(icon).toHaveClass('spinning');
    rerender(<RefreshButton refreshing={false} />); expect(screen.getByRole('button')).not.toBeDisabled(); expect(icon).not.toHaveClass('spinning');
  });
  it('does not expire feedback on a frontend timer', () => {
    vi.useFakeTimers(); try {
      const { rerender } = render(<StatusBanner feedback={{ text: '额度已刷新', kind: 'success', caption: '刚刚更新' }} />);
      vi.advanceTimersByTime(60000); expect(screen.getByRole('status')).toHaveTextContent('额度已刷新');
      rerender(<></>); expect(screen.queryByRole('status')).toBeNull();
    } finally { vi.useRealTimers(); }
  });
  it('disables both download actions and preserves progress text', () => {
    const release = { version: '2.0.0', asset_url: '', signature_url: '', page_url: '' };
    render(<UpdateBanner update={{ status: 'downloading', release, percent: 42, received: 42, total: 100 }} />);
    expect(screen.getByText('正在下载更新… 42%')).toBeVisible(); expect(screen.getByText('42 B / 100 B')).toBeVisible();
    expect(screen.getByRole('button', { name: '稍后' })).toBeDisabled(); expect(screen.getByRole('button', { name: '立即更新' })).toBeDisabled();
  });
});
describe('operation lock regression matrix', () => {
  it.each([
    ['refresh', { refreshing: true }, true, true, true],
    ['switch', { switchingTo: 'target' }, true, true, true],
    ['adding', { addingAccount: true }, true, true, false],
    ['readonly', { persistenceBlocked: true }, false, true, false],
  ] as const)('%s locks', (_, patch, switchDisabled, addDisabled, cancelDisabled) => {
    const result = operationLocks({ ...state(), ...patch }); expect(result.switchAccount).toBe(switchDisabled); expect(result.addAccount).toBe(addDisabled); expect(result.cancelLogin).toBe(cancelDisabled);
  });
  it.each(['downloading', 'ready'] as const)('locks mutation actions during %s', status => {
    const view = state(); view.update = status === 'ready' ? { status, prepared: '', release: { version: '', asset_url: '', signature_url: '', page_url: '' } } : { status, percent: 0, received: 0, total: null, release: { version: '', asset_url: '', signature_url: '', page_url: '' } };
    expect(operationLocks(view)).toMatchObject({ switchAccount: true, addAccount: true, reimport: true, cancelLogin: false });
  });
  it('matches five known UTF-8 avatar palette selections', () => {
    expect(['alex.chen@gmail.com', 'jamie.work@outlook.com', 'alex.creative@icloud.com', 'a@example.test', '测试@example.test'].map(email => stableHash(email) % 3)).toEqual([2, 0, 1, 1, 0]);
  });
});
describe('screen boundaries', () => {
  it('offers one add action for an empty list', () => { const view = state(); view.accounts = []; render(<MainScreen state={view} {...actions()} />); expect(screen.getAllByRole('button', { name: '添加账号' })).toHaveLength(1); });
  it('shows readonly import controls as disabled', () => { const view = state(); view.persistenceBlocked = true; view.unregisteredLiveEmail = 'unregistered@example.test'; render(<MainScreen state={view} {...actions()} />); expect(screen.getByRole('button', { name: '导入' })).toBeDisabled(); });
  it('navigates to account management from settings', () => { const handlers = actions(); render(<SettingsScreen state={state()} {...handlers} />); fireEvent.click(screen.getByRole('button', { name: '3 个账号' })); expect(handlers.navigate).toHaveBeenCalledWith('manage'); });
  it('never allows removal of the live current record', () => { const remove = vi.fn(); render(<ManageScreen state={state()} {...actions()} onRemove={remove} />); const button = screen.getByRole('button', { name: '当前账号不可移除' }); button.click(); expect(button).toBeDisabled(); expect(remove).not.toHaveBeenCalled(); });
  it('orders destructive confirmation before cancel and traps keyboard focus', () => {
    const accept = vi.fn(); const cancel = vi.fn(); render(<ConfirmDialog email="a@example.test" disabled={false} onConfirm={accept} onCancel={cancel} />);
    const buttons = screen.getAllByRole('button'); expect(buttons[0]).toHaveTextContent('移除 a@example.test'); expect(buttons[1]).toHaveTextContent('取消'); expect(buttons[1]).toHaveFocus();
    fireEvent.keyDown(buttons[1], { key: 'Tab' }); expect(buttons[0]).toHaveFocus(); fireEvent.click(buttons[1]); expect(cancel).toHaveBeenCalledTimes(1); expect(accept).not.toHaveBeenCalled();
  });
});
