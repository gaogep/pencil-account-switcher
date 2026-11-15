import type { ComponentChildren } from 'preact';
import { Plus, RefreshCw, RotateCw, X } from 'lucide-preact';
import type { AuthState } from '../generated/AuthState';
import type { ViewState } from '../generated/ViewState';

export const authLabels: Record<AuthState, string> = { ok: '生效中', expired: '已失效', unknown: '待确认' };
export function operationLocks(state: ViewState) {
  const switching = state.switchingTo !== null;
  const updateBusy = state.update.status === 'downloading' || state.update.status === 'ready';
  const locked = state.refreshing || switching || updateBusy;
  return {
    switchAccount: locked || state.addingAccount,
    addAccount: locked || state.addingAccount || state.persistenceBlocked,
    reimport: locked || state.addingAccount || state.persistenceBlocked,
    cancelLogin: state.refreshing || switching,
    removeAccount: switching || state.addingAccount || state.persistenceBlocked,
    importCurrent: state.persistenceBlocked,
    refresh: state.refreshing,
  };
}
export type ButtonProps = { children: ComponentChildren; onClick?: () => void; disabled?: boolean; label?: string; title?: string; icon?: ComponentChildren };
export function LinkButton({ children, onClick, disabled, label, title, icon }: ButtonProps) {
  return <button type="button" class="link-button" onClick={onClick} disabled={disabled} aria-label={label} title={title}>{icon}{children}</button>;
}
export function PillButton({ children, onClick, disabled, label, title, icon, variant = 'primary' }: ButtonProps & { variant?: 'primary' | 'remove' | 'muted' }) {
  return <button type="button" class={`pill-button pill-${variant}`} onClick={onClick} disabled={disabled} aria-label={label} title={title}>{icon}{children}</button>;
}
export function QuietActionButton({ children, onClick, disabled, label, title, icon }: ButtonProps) {
  return <button type="button" class="quiet-button" onClick={onClick} disabled={disabled} aria-label={label} title={title}>{icon}<span class="quiet-label">{children}</span></button>;
}
export function IconButton({ label, onClick, disabled, children }: { label: string; onClick?: () => void; disabled?: boolean; children: ComponentChildren }) {
  return <button type="button" class="icon-button" onClick={onClick} disabled={disabled} aria-label={label} title={label}>{children}</button>;
}
export function RefreshButton({ refreshing, onClick, disabled = false }: { refreshing: boolean; onClick?: () => void; disabled?: boolean }) {
  return <button type="button" class="icon-button refresh-button" onClick={onClick} disabled={disabled || refreshing} aria-label="刷新额度" title="刷新额度" aria-busy={refreshing}><RotateCw class={refreshing ? 'spinning' : ''} size={15} strokeWidth={1.5} aria-hidden="true" /></button>;
}
export function AddAccountButton({ adding, disabled, onClick }: { adding: boolean; disabled?: boolean; onClick?: () => void }) {
  const Icon = adding ? X : Plus;
  return <button type="button" class="outline-button" onClick={onClick} disabled={disabled}><Icon size={14} strokeWidth={1.5} aria-hidden="true" />{adding ? '取消网页登录' : '添加账号'}</button>;
}
export function ReimportButton({ email, disabled, onClick }: { email: string; disabled?: boolean; onClick?: () => void }) {
  return <QuietActionButton onClick={onClick} disabled={disabled} label={`重新导入 ${email}`} title={`登录已失效，打开网页重新登录 ${email}，无需先移除`} icon={<RefreshCw size={11} strokeWidth={1.5} aria-hidden="true" />}>重新导入</QuietActionButton>;
}
export function InkSwitch({ checked, label, disabled, onChange }: { checked: boolean; label: string; disabled?: boolean; onChange: (checked: boolean) => void }) {
  return <button type="button" role="switch" aria-checked={checked} aria-label={label} class={`ink-switch ${checked ? 'is-on' : ''}`} disabled={disabled} onClick={() => onChange(!checked)}><span aria-hidden="true" /></button>;
}
export function AuthBadge({ state }: { state: AuthState }) { return <span class={`auth-badge auth-${state}`}>{authLabels[state]}</span>; }
export function quotaText(used?: number, total?: number) { return used === undefined || total === undefined ? '— / — 天' : `${used} / ${total} 天`; }
export function QuotaProgressBar({ used, total, fraction, current = false, width, label = 'MCP 使用天数' }: { used?: number; total?: number; fraction?: number; current?: boolean; width?: number; label?: string }) {
  const raw = fraction ?? (used !== undefined && total !== undefined && total > 0 ? used / total : undefined);
  const value = raw !== undefined && Number.isFinite(raw) ? Math.min(1, Math.max(0, raw)) : undefined;
  return <div class={`quota-track ${current ? 'quota-current' : ''}`} style={{ width: width ? `${width}px` : undefined }} role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={value !== undefined ? value * 100 : undefined} aria-valuetext={fraction !== undefined ? undefined : quotaText(used, total)}><span style={{ width: `${(value ?? 0) * 100}%` }} /></div>;
}
export function stableHash(email: string): number { let hash = 5381; for (const byte of new TextEncoder().encode(email)) hash = (hash * 33 + byte) & 0x7fffffff; return hash; }
export function Avatar({ email, size = 34 }: { email: string; size?: 34 | 36 }) {
  const local = email.split('@').filter(Boolean)[0] ?? email;
  const letters = Array.from(new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(local), item => item.segment).slice(0, 2).join('').toUpperCase();
  const palette = stableHash(email) % 3;
  return <span class="avatar" aria-hidden="true" style={{ width: `${size}px`, height: `${size}px`, fontSize: `${size === 36 ? 12 : 11}px`, background: `var(--avatar-${palette}-bg)`, color: `var(--avatar-${palette}-fg)` }}>{letters}</span>;
}
export function Hairline() { return <div class="hairline" aria-hidden="true" />; }
