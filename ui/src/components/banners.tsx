import { CircleAlert, CircleArrowDown, CircleCheck, RotateCw } from 'lucide-preact';
import type { Feedback } from '../generated/Feedback';
import type { UpdateState } from '../generated/UpdateState';
import { LinkButton, PillButton, QuotaProgressBar } from './primitives';
export function StatusBanner({ feedback }: { feedback: Feedback }) {
  const Icon = feedback.kind === 'success' ? CircleCheck : feedback.kind === 'loading' ? RotateCw : CircleAlert;
  return <div class={`status-slot status-${feedback.kind}`}><div class="status-banner" role={feedback.kind === 'warning' ? 'alert' : 'status'} title={feedback.text} aria-label={feedback.caption ? `${feedback.text}，${feedback.caption}` : feedback.text}><Icon size={15} strokeWidth={1.5} aria-hidden="true" /><strong>{feedback.text}</strong>{feedback.caption && <span>{feedback.caption}</span>}</div></div>;
}
export function formatBytes(value: number): string { if (value < 1024) return `${value} B`; const kb = value / 1024; return kb < 1024 ? `${kb.toFixed(1)} KB` : `${(kb / 1024).toFixed(1)} MB`; }
export function UpdateBanner({ update, onPostpone, onStart, onInstall }: { update: UpdateState; onPostpone?: () => void; onStart?: () => void; onInstall?: () => void }) {
  if (update.status === 'idle' || (update.status === 'available' && update.silenced)) return null;
  const downloading = update.status === 'downloading';
  const ready = update.status === 'ready';
  const title = ready ? '已下载，将重启以完成更新' : downloading ? `正在下载更新… ${update.percent}%` : `新版本 ${update.release.version} 可用`;
  const caption = ready ? '会替换应用程序文件夹里的副本' : downloading ? update.total !== null ? `${formatBytes(update.received)} / ${formatBytes(update.total)}` : formatBytes(update.received) : '下载完成后会重启应用';
  const Icon = ready ? CircleCheck : downloading ? RotateCw : CircleArrowDown;
  return <div class="update-slot"><div class="update-banner" role="status" aria-label={title}><Icon size={15} strokeWidth={1.5} aria-hidden="true" /><div class="update-copy"><strong title={title}>{title}</strong><span title={caption}>{caption}</span>{downloading && update.total !== null && <QuotaProgressBar fraction={update.percent / 100} width={160} label="下载更新" />}</div><div class="update-actions">{!ready && <LinkButton onClick={onPostpone} disabled={downloading}>稍后</LinkButton>}<PillButton onClick={ready ? onInstall : onStart} disabled={downloading} variant={downloading ? 'muted' : 'primary'}>{ready ? '安装并重启' : '立即更新'}</PillButton></div></div></div>;
}
