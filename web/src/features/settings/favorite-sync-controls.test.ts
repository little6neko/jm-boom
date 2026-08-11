import { describe, expect, test } from 'bun:test'

import type { FavoriteSyncState } from '@/lib/api/favorite-sync'
import {
  favoriteSyncDescription,
  favoriteSyncDifferenceSummary,
  favoriteSyncProgress,
  favoriteSyncProgressLabel,
  favoriteSyncStatusLabel
} from './favorite-sync-controls'

describe('favorite sync settings presentation', () => {
  test('explains that disabled synchronization does not access remote favorites', () => {
    expect(favoriteSyncDescription(undefined, false)).toContain('默认不会访问远端收藏')
    expect(favoriteSyncDescription(undefined, true)).toContain('开启时先检查两端差异')
  })

  test('maps actionable states and bounds progress', () => {
    expect(favoriteSyncStatusLabel('needsResolution')).toBe('需要处理差异')
    expect(favoriteSyncStatusLabel('error')).toBe('同步失败')
    expect(favoriteSyncProgress(syncState({ progressDone: 5, progressTotal: 10 }))).toBe(50)
    expect(favoriteSyncProgress(syncState({ progressDone: 12, progressTotal: 10 }))).toBe(100)
    expect(favoriteSyncProgress(syncState({ progressDone: 0, progressTotal: 0 }))).toBe(0)
  })

  test('explains which side contains each favorite difference', () => {
    expect(
      favoriteSyncDifferenceSummary(
        syncState({ localCount: 337, remoteCount: 336, localOnlyCount: 2, remoteOnlyCount: 1 })
      )
    ).toBe('本地收藏 337 项，远端收藏 336 项；其中 2 项只存在于本地，1 项只存在于远端。')
  })

  test('labels each merge phase without changing the checking message', () => {
    expect(
      favoriteSyncProgressLabel(
        syncState({ status: 'checking', pendingKind: 'check', progressPhase: null })
      )
    ).toBe('正在读取并比较两端收藏')
    expect(
      favoriteSyncProgressLabel(
        syncState({ pendingKind: 'merge', progressPhase: 'fetchingRemote' })
      )
    ).toBe('正在拉取远端收藏')
    expect(
      favoriteSyncProgressLabel(
        syncState({ pendingKind: 'merge', progressPhase: 'uploadingLocal' })
      )
    ).toBe('正在上传本地收藏')
    expect(
      favoriteSyncProgressLabel(syncState({ pendingKind: 'merge', progressPhase: 'verifying' }))
    ).toBe('正在检查同步结果')
    expect(favoriteSyncProgressLabel(syncState({ progressPhase: null }))).toBe('正在同步收藏')
  })
})

function syncState(overrides: Partial<FavoriteSyncState>): FavoriteSyncState {
  return {
    enabled: true,
    status: 'syncing',
    accountUsername: 'tester',
    localCount: 0,
    remoteCount: 0,
    localOnlyCount: 0,
    remoteOnlyCount: 0,
    progressDone: 0,
    progressTotal: 0,
    progressPhase: null,
    pendingKind: null,
    pendingComicId: null,
    pendingTarget: null,
    lastError: null,
    lastCheckedAt: null,
    lastSyncedAt: null,
    ...overrides
  }
}
