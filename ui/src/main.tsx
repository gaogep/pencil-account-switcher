import { render } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';
import { invoke, listen } from './ipc';
import type { ViewState } from './generated/ViewState';
import './theme.css';
import './components/components.css';
import { BrandHeader } from './components';
import { ComponentGallery } from './ComponentGallery';

function App() {
  const [state, setState] = useState<ViewState | null>(null);
  const root = useRef<HTMLElement>(null);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen('state', next => { if (!disposed) setState(next); }).then(stop => {
      if (disposed) stop(); else unlisten = stop;
      if (!disposed) void invoke({ command: 'get_state' }).then(next => { if (!disposed) setState(next); });
    });
    return () => { disposed = true; unlisten?.(); };
  }, []);
  useEffect(() => {
    const element = root.current;
    if (!element) return;
    const observer = new ResizeObserver(() => { void invoke({ command: 'set_panel_height', args: { h: element.getBoundingClientRect().height } }); });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    const escape = (event: KeyboardEvent) => { if (event.key === 'Escape') void invoke({ command: 'hide_panel' }); };
    document.addEventListener('keydown', escape);
    return () => document.removeEventListener('keydown', escape);
  }, []);
  return <main class="panel" ref={root} aria-busy={!state}>{state && import.meta.env.MODE === 'mock' && new URLSearchParams(location.search).has('gallery') ? <ComponentGallery state={state} /> : <BrandHeader />}</main>;
}
render(<App />, document.getElementById('app')!);
