import { useEffect, useRef, useState } from 'preact/hooks';
import { invoke, listen, loadSnapshot, mockMode } from './ipc';
import type { ViewState } from './generated/ViewState';
import type { IpcRequest } from './generated/IpcRequest';
import { BrandHeader, StatusBanner, operationLocks } from './components';
import { ComponentGallery } from './ComponentGallery';
import { ConfirmDialog } from './ConfirmDialog';
import { MainScreen, SettingsScreen, ManageScreen, type Route } from './screens';
export function App() {
  const [state, setState] = useState<ViewState | null>(null);
  const [route, navigate] = useState<Route>('main'); const [confirmId, confirm] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null); const root = useRef<HTMLElement>(null);
  const send = (request: IpcRequest) => { void invoke(request).catch(e => setError(String(e))); };
  useEffect(() => {
    let disposed = false; let revision = 0; const stops: (() => void)[] = [];
    const addStop = (stop: () => void) => { if (disposed) stop(); else stops.push(stop); };
    const report = (e: unknown) => { if (!disposed) setError(String(e)); };
    void listen('panel-shown', () => { if (!disposed) { navigate('main'); confirm(null); } }).then(addStop).catch(report);
    void listen('state', next => { revision++; if (!disposed) { setState(next); setError(null); } }).then(async stop => {
      addStop(stop); if (disposed) return;
      const before = revision; const next = await invoke({ command: 'get_state' });
      if (!disposed && revision === before) setState(next);
      if (mockMode) {
        const snapshot = await loadSnapshot();
        if (!disposed) { navigate(snapshot.route); confirm(snapshot.state.accounts.find(a => a.email === snapshot.confirmRemoveEmail)?.id ?? null); }
      }
    }).catch(report);
    return () => { disposed = true; stops.forEach(stop => stop()); };
  }, []);
  useEffect(() => {
    send({ command: 'set_confirmation_open', args: { open: confirmId !== null } });
    return () => { void invoke({ command: 'set_confirmation_open', args: { open: false } }).catch(() => {}); };
  }, [confirmId]);
  useEffect(() => {
    const element = root.current; if (!element) return; let previous = 0;
    const observer = new ResizeObserver(() => { const h = Math.ceil(element.getBoundingClientRect().height); if (h !== previous) { previous = h; send({ command: 'set_panel_height', args: { h } }); } });
    observer.observe(element); return () => observer.disconnect();
  }, []);
  useEffect(() => {
    const escape = (event: KeyboardEvent) => { if (event.key !== 'Escape') return; event.preventDefault(); if (confirmId) confirm(null); else send({ command: 'hide_panel' }); };
    document.addEventListener('keydown', escape); return () => document.removeEventListener('keydown', escape);
  }, [confirmId]);
  const account = state?.accounts.find(a => a.id === confirmId);
  useEffect(() => { if (state && confirmId && !account) confirm(null); }, [state, confirmId, account]);
  const gallery = mockMode && new URLSearchParams(location.search).has('gallery');
  return <main class="panel" ref={root} aria-busy={!state}>
    <div inert={account !== undefined}>{error && <StatusBanner feedback={{ text: error, kind: 'warning', caption: null }} />}{!state ? <BrandHeader /> : gallery ? <ComponentGallery state={state} /> : route === 'main' ? <MainScreen state={state} send={send} navigate={navigate} /> : route === 'settings' ? <SettingsScreen state={state} send={send} navigate={navigate} /> : <ManageScreen state={state} send={send} navigate={navigate} onRemove={confirm} />}</div>
    {account && <ConfirmDialog email={account.email} disabled={account.isCurrent || !state || operationLocks(state).removeAccount} onCancel={() => confirm(null)} onConfirm={() => { if (!account.isCurrent) { send({ command: 'remove_account', args: { id: account.id } }); confirm(null); } }} />}
  </main>;
}
