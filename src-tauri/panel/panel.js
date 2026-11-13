document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') window.__TAURI__.core.invoke('hide_panel');
});
