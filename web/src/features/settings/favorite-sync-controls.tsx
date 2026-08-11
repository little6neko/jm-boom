import {
  CloudIcon,
  GitMergeIcon,
  LoaderCircleIcon,
  RefreshCwIcon,
  ServerIcon,
  TriangleAlertIcon
} from 'lucide-react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Switch } from '@/components/ui/switch'
import type {
  FavoriteSyncResolution,
  FavoriteSyncState,
  FavoriteSyncStatus
} from '@/lib/api/favorite-sync'
import { SettingRow } from './shared'

export function FavoriteSyncControls({
  state,
  accountLoggedIn,
  isLoading,
  isMutating,
  onEnabledChange,
  onCheck,
  onResolve,
  onRetry
}: {
  state: FavoriteSyncState | undefined
  accountLoggedIn: boolean
  isLoading: boolean
  isMutating: boolean
  onEnabledChange: (enabled: boolean) => void
  onCheck: () => void
  onResolve: (resolution: FavoriteSyncResolution) => void
  onRetry: () => void
}) {
  const enabled = state?.enabled ?? false
  const switchDisabled = isLoading || isMutating || (!enabled && !accountLoggedIn)

  return (
    <div className="space-y-3 border-t pt-5">
      <SettingRow
        title="同步收藏"
        description={favoriteSyncDescription(state, accountLoggedIn)}
        inline
      >
        <Switch checked={enabled} disabled={switchDisabled} onCheckedChange={onEnabledChange} />
      </SettingRow>

      {enabled && state ? (
        <FavoriteSyncDetails
          state={state}
          disabled={isMutating}
          onCheck={onCheck}
          onResolve={onResolve}
          onRetry={onRetry}
        />
      ) : null}
    </div>
  )
}

function FavoriteSyncDetails({
  state,
  disabled,
  onCheck,
  onResolve,
  onRetry
}: {
  state: FavoriteSyncState
  disabled: boolean
  onCheck: () => void
  onResolve: (resolution: FavoriteSyncResolution) => void
  onRetry: () => void
}) {
  const progress = favoriteSyncProgress(state)

  return (
    <div className="space-y-3 rounded-lg border bg-muted/30 p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <Badge variant={state.status === 'error' ? 'destructive' : 'outline'} className="h-7 px-3">
          {statusIcon(state.status)}
          {favoriteSyncStatusLabel(state.status)}
        </Badge>
        {state.accountUsername ? (
          <span className="text-xs text-muted-foreground">账号：{state.accountUsername}</span>
        ) : null}
      </div>

      {state.status === 'needsResolution' ? (
        <div className="space-y-3">
          <p className="text-xs leading-5 text-muted-foreground">
            {favoriteSyncDifferenceSummary(state)}
          </p>
          <div className="flex flex-wrap justify-end gap-2">
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={disabled}
              onClick={() => onResolve('remoteOverwrite')}
            >
              <ServerIcon className="size-4" />
              使用远端覆盖本地
            </Button>
            <Button type="button" size="sm" disabled={disabled} onClick={() => onResolve('merge')}>
              <GitMergeIcon className="size-4" />
              合并两端
            </Button>
          </div>
        </div>
      ) : null}

      {state.status === 'checking' || state.status === 'syncing' ? (
        <div className="space-y-2">
          <div className="flex justify-between gap-3 text-xs text-muted-foreground">
            <span>{favoriteSyncProgressLabel(state)}</span>
            {state.progressTotal > 0 ? (
              <span>
                {state.progressDone}/{state.progressTotal}
              </span>
            ) : null}
          </div>
          <Progress value={progress} className="h-2" />
        </div>
      ) : null}

      {state.status === 'synced' ? (
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-xs text-muted-foreground">
            两端共 {state.localCount} 项
            {state.lastSyncedAt ? ` · ${formatSyncTime(state.lastSyncedAt)}` : ''}
          </p>
          <Button type="button" size="sm" variant="outline" disabled={disabled} onClick={onCheck}>
            <RefreshCwIcon className="size-4" />
            重新检查
          </Button>
        </div>
      ) : null}

      {state.status === 'error' ? (
        <div className="space-y-3">
          <p className="text-xs leading-5 text-destructive">
            {state.lastError ?? '收藏同步失败'}
            {state.pendingComicId ? `（漫画 ${state.pendingComicId}）` : ''}
          </p>
          <div className="flex justify-end">
            <Button type="button" size="sm" disabled={disabled} onClick={onRetry}>
              <RefreshCwIcon className="size-4" />
              重试同步
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  )
}

export function favoriteSyncStatusLabel(status: FavoriteSyncStatus) {
  if (status === 'checking') return '正在检查'
  if (status === 'needsResolution') return '需要处理差异'
  if (status === 'syncing') return '正在同步'
  if (status === 'synced') return '已同步'
  if (status === 'error') return '同步失败'
  return '未开启'
}

export function favoriteSyncDescription(
  state: FavoriteSyncState | undefined,
  accountLoggedIn: boolean
) {
  if (state?.enabled) return '开启后，在本服务中收藏或取消收藏都会同步到远端账号'
  if (!accountLoggedIn) return '登录 JM 账号后可开启；默认不会访问远端收藏'
  return '开启时先检查两端差异，完成后保持单项收藏同步'
}

export function favoriteSyncDifferenceSummary(state: FavoriteSyncState) {
  return `本地收藏 ${state.localCount} 项，远端收藏 ${state.remoteCount} 项；其中 ${state.localOnlyCount} 项只存在于本地，${state.remoteOnlyCount} 项只存在于远端。`
}

export function favoriteSyncProgress(state: FavoriteSyncState) {
  if (state.progressTotal <= 0) return 0
  return Math.min(100, Math.round((state.progressDone / state.progressTotal) * 100))
}

export function favoriteSyncProgressLabel(state: FavoriteSyncState) {
  if (state.status === 'checking') return '正在读取并比较两端收藏'
  if (state.status !== 'syncing' || state.pendingKind !== 'merge') return '正在同步收藏'
  if (state.progressPhase === 'fetchingRemote') return '正在拉取远端收藏'
  if (state.progressPhase === 'uploadingLocal') return '正在上传本地收藏'
  if (state.progressPhase === 'verifying') return '正在检查同步结果'
  return '正在同步收藏'
}

function statusIcon(status: FavoriteSyncStatus) {
  if (status === 'checking' || status === 'syncing') {
    return <LoaderCircleIcon className="size-3.5 animate-spin" />
  }
  if (status === 'error') return <TriangleAlertIcon className="size-3.5" />
  return <CloudIcon className="size-3.5" />
}

function formatSyncTime(timestamp: number) {
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  }).format(timestamp)
}
