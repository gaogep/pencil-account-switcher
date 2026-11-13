import type { IpcRequest } from './generated/IpcRequest';
import type { IpcEvents } from './generated/IpcEvents';
import type { ViewState } from './generated/ViewState';
export type Snapshot = { state: ViewState; route: 'main' | 'settings' | 'manage'; confirmRemoveEmail: string | null };
export const mockMode = import.meta.env.MODE === 'mock' || new URLSearchParams(location.search).has('mock');
let snapshot: Promise<Snapshot> | undefined;
export function loadSnapshot(): Promise<Snapshot> {
  const name = new URLSearchParams(location.search).get('mock') || 'main';
  if (!/^[a-z-]+$/.test(name)) return Promise.reject(new Error('Unknown scenario'));
  return snapshot ??= fetch(`/scenarios/${name}.json`).then(async response => {
    if (!response.ok) throw new Error(`Scenario unavailable: ${name}`);
    return response.json() as Promise<Snapshot>;
  });
}
export async function invoke<R extends IpcRequest>(request: R): Promise<R extends { command: 'get_state' } ? ViewState : void> {
  type Result = R extends { command: 'get_state' } ? ViewState : void;
  if (mockMode) return (request.command === 'get_state' ? (await loadSnapshot()).state : undefined) as Result;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<Result>(request.command, 'args' in request ? request.args : {});
}
export async function listen<K extends keyof IpcEvents>(event: K, handler: (payload: IpcEvents[K]) => void): Promise<() => void> {
  if (mockMode) {
    if (event === 'state') handler((await loadSnapshot()).state as IpcEvents[K]);
    return () => {};
  }
  const { listen } = await import('@tauri-apps/api/event');
  return listen<IpcEvents[K]>(event, incoming => handler(incoming.payload));
}
