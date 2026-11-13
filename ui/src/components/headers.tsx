import type { ComponentChildren } from 'preact';
import { ChevronLeft } from 'lucide-preact';
export function BrandHeader({ actions }: { actions?: ComponentChildren }) {
  return <header class="brand-header"><img src="/brand.png" width="32" height="32" alt="" /><div class="brand-heading"><h1>Pencil Account Switcher</h1><p>账号切换器</p></div><div class="header-actions">{actions}</div></header>;
}
export function NavHeader({ title, subtitle, backLabel, onBack, trailing }: { title: string; subtitle: string; backLabel: string; onBack?: () => void; trailing?: ComponentChildren }) {
  return <header class="nav-header"><button type="button" class="back-button" onClick={onBack} title={`返回${backLabel}`} aria-label={`返回${backLabel}`}><ChevronLeft size={14} strokeWidth={1.5} aria-hidden="true" /></button><div class="nav-heading"><h1>{title}</h1><p>{subtitle}</p></div><div class="header-actions">{trailing}</div></header>;
}
