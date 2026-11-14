import { useEffect, useRef } from 'preact/hooks';
export function ConfirmDialog({ email, disabled, onConfirm, onCancel }: { email: string; disabled: boolean; onConfirm: () => void; onCancel: () => void }) {
  const dialog = useRef<HTMLDivElement>(null); const cancel = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const previous = document.activeElement; cancel.current?.focus();
    return () => { if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, []);
  return <div class="dialog-backdrop"><div ref={dialog} class="confirm-dialog" role="alertdialog" aria-modal="true" aria-labelledby="remove-title" aria-describedby="remove-description" onKeyDown={event => {
    if (event.key !== 'Tab') return;
    const buttons = Array.from(dialog.current?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? []);
    const first = buttons[0]; const last = buttons.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }}><h2 id="remove-title">从切号器移除此账号？</h2><p id="remove-description">将删除 {email} 在本机的登录凭据与额度缓存；远端账号不受影响。</p><div class="dialog-actions"><button type="button" class="pill-button pill-primary" disabled={disabled} onClick={onConfirm}>移除 {email}</button><button ref={cancel} type="button" class="quiet-button" onClick={onCancel}>取消</button></div></div></div>;
}
