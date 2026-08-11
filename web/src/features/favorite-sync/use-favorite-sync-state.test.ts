import { describe, expect, test } from 'bun:test'

import type { FavoriteSyncState } from '@/lib/api/favorite-sync'
import { favoriteSyncRefetchInterval } from './use-favorite-sync-state'

describe('favoriteSyncRefetchInterval', () => {
  test('polls only while a background synchronization task is active', () => {
    expect(favoriteSyncRefetchInterval(syncState('checking'))).toBe(1000)
    expect(favoriteSyncRefetchInterval(syncState('syncing'))).toBe(1000)
    expect(favoriteSyncRefetchInterval(syncState('needsResolution'))).toBe(false)
    expect(favoriteSyncRefetchInterval(syncState('error'))).toBe(false)
    expect(favoriteSyncRefetchInterval(undefined)).toBe(false)
  })
})

function syncState(status: FavoriteSyncState['status']): FavoriteSyncState {
  return {
    enabled: status !== 'disabled',
    status,
    accountUsername: null,
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
    lastSyncedAt: null
  }
}
