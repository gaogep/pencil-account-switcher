import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/preact';
import fixture from '../../public/scenarios/main.json';
import type { ViewState } from '../generated/ViewState';
import { App } from '../App';
const ipc = vi.hoisted(() => ({ invoke: vi.fn(), stops: [] as ReturnType<typeof vi.fn>[], events: new Map<string, (payload: any) => void>() }));
vi.mock('../ipc', () => ({ mockMode: false, invoke: ipc.invoke, listen: async (name: string, handler: (payload: any) => void) => { ipc.events.set(name, handler); const stop = vi.fn(); ipc.stops.push(stop); return stop; } }));
const state = () => structuredClone(fixture.state) as ViewState;
let measurements: Map<Element, () => void>;
beforeEach(() => {
  measurements = new Map();
  ipc.invoke.mockReset(); ipc.events.clear(); ipc.stops.length = 0;
  ipc.invoke.mockImplementation(async request => request.command === 'get_state' ? state() : undefined);
  vi.stubGlobal('ResizeObserver', class { constructor(private callback: () => void) {} observe(node: Element) { measurements.set(node, this.callback); } disconnect() {} });
});
describe('application IPC and routing', () => {
  it('opens confirmation, Escape closes only confirmation, and reopening panel resets the route', async () => {
    render(<App />); await screen.findByRole('button', { name: '设置' });
    fireEvent.click(screen.getByRole('button', { name: '设置' })); fireEvent.click(screen.getByRole('button', { name: '3 个账号' }));
    fireEvent.click(screen.getByRole('button', { name: `移除 ${state().accounts[1].email}` }));
    expect(screen.getByRole('alertdialog')).toHaveTextContent('从切号器移除此账号？');
    await waitFor(() => expect(ipc.invoke).toHaveBeenCalledWith({ command: 'set_confirmation_open', args: { open: true } }));
    fireEvent.keyDown(document, { key: 'Escape' }); expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(ipc.invoke).not.toHaveBeenCalledWith({ command: 'hide_panel' });
    act(() => ipc.events.get('panel-shown')?.(undefined)); expect(screen.getByRole('button', { name: '设置' })).toBeVisible();
    fireEvent.keyDown(document, { key: 'Escape' }); expect(ipc.invoke).toHaveBeenCalledWith({ command: 'hide_panel' });
  });
  it('sends the selected account ID only after confirmation', async () => {
    render(<App />); await screen.findByRole('button', { name: '设置' });
    fireEvent.click(screen.getByRole('button', { name: '设置' })); fireEvent.click(screen.getByRole('button', { name: '3 个账号' }));
    const account = state().accounts[1]; fireEvent.click(screen.getByRole('button', { name: `移除 ${account.email}` }));
    expect(ipc.invoke).not.toHaveBeenCalledWith({ command: 'remove_account', args: { id: account.id } });
    fireEvent.click(screen.getByRole('alertdialog').querySelector('button')!);
    expect(ipc.invoke).toHaveBeenCalledWith({ command: 'remove_account', args: { id: account.id } });
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });
  it('recovers when a confirmed account disappears in a newer state', async () => {
    render(<App />); await screen.findByRole('button', { name: '设置' });
    fireEvent.click(screen.getByRole('button', { name: '设置' })); fireEvent.click(screen.getByRole('button', { name: '3 个账号' }));
    fireEvent.click(screen.getByRole('button', { name: `移除 ${state().accounts[1].email}` }));
    const next = state(); next.accounts.splice(1, 1); act(() => ipc.events.get('state')?.(next));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    expect(document.querySelector('[inert]')).toBeNull();
    await waitFor(() => expect(ipc.invoke).toHaveBeenLastCalledWith({ command: 'set_confirmation_open', args: { open: false } }));
  });
  it('does not overwrite a newer state event with a delayed get_state reply', async () => {
    let reply: (value: ViewState) => void = () => {};
    ipc.invoke.mockImplementation(request => request.command === 'get_state' ? new Promise<ViewState>(resolve => { reply = resolve; }) : Promise.resolve());
    render(<App />); await waitFor(() => expect(ipc.invoke).toHaveBeenCalledWith({ command: 'get_state' }));
    const next = state(); next.feedback = { text: '新的状态', kind: 'success', caption: null };
    act(() => ipc.events.get('state')?.(next)); await act(async () => reply(state()));
    expect(screen.getByRole('status')).toHaveTextContent('新的状态');
    next.feedback = null; act(() => ipc.events.get('state')?.(structuredClone(next))); expect(screen.queryByRole('status')).toBeNull();
  });
  it('reports each measured height once and removes IPC subscriptions on unmount', async () => {
    const view = render(<App />); await screen.findByRole('button', { name: '设置' });
    const spy = vi.spyOn(screen.getByRole('main'), 'getBoundingClientRect').mockReturnValue({ height: 450 } as DOMRect);
    act(() => { const measure = measurements.get(screen.getByRole('main'))!; measure(); measure(); }); expect(ipc.invoke.mock.calls.filter(([r]) => r.command === 'set_panel_height')).toEqual([[{ command: 'set_panel_height', args: { h: 450 } }]]);
    spy.mockRestore(); view.unmount(); await waitFor(() => expect(ipc.stops.map(stop => stop.mock.calls.length)).toEqual([1, 1]));
  });
});
