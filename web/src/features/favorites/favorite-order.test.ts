import { describe, expect, test } from 'bun:test'

import type { FavoriteSyncState } from '@/lib/api/favorite-sync'
import { ApiError } from '@/lib/api/client'
import { queryKeys } from '@/lib/query-keys'
import {
  favoriteOrderErrorCode,
  favoriteOrderOptions,
  isFavoriteSyncReady,
  parseFavoriteOrder,
  shouldRetryFavoriteOrder
} from './favorite-order'

describe('favorite ordering', () => {
  test('accepts mp and falls back every other URL value to mr', () => {
    expect(parseFavoriteOrder('mr')).toBe('mr')
    expect(parseFavoriteOrder('mp')).toBe('mp')
    expect(parseFavoriteOrder('unknown')).toBe('mr')
    expect(parseFavoriteOrder(undefined)).toBe('mr')
  })

  test('enables update ordering only after synchronization completes', () => {
    expect(isFavoriteSyncReady(syncState('synced'))).toBe(true)
    expect(isFavoriteSyncReady(syncState('checking'))).toBe(false)
    expect(isFavoriteSyncReady(syncState('disabled'))).toBe(false)
    expect(isFavoriteSyncReady(undefined)).toBe(false)
    expect(favoriteOrderOptions(false)[1].disabled).toBe(true)
    expect(favoriteOrderOptions(true)[1].disabled).toBe(false)
  })

  test('recognizes stable ordering error codes and retries stale reads once', () => {
    const stale = apiError('favorite_order_stale')
    expect(favoriteOrderErrorCode(stale)).toBe('favorite_order_stale')
    expect(shouldRetryFavoriteOrder(0, stale)).toBe(true)
    expect(shouldRetryFavoriteOrder(1, stale)).toBe(false)
    expect(favoriteOrderErrorCode(apiError('unknown'))).toBeUndefined()
    expect(favoriteOrderErrorCode(new Error('network'))).toBeUndefined()
  })

  test('keeps page and order in separate favorite list cache entries', () => {
    expect(queryKeys.favorites(2, 'mr')).toEqual(['jm-favorites', 2, 'mr'])
    expect(queryKeys.favorites(2, 'mp')).toEqual(['jm-favorites', 2, 'mp'])
    expect(queryKeys.favorites()).toEqual(['jm-favorites'])
  })
})

function apiError(code: string) {
  return new ApiError('favorite order failed', 409, { code }, false)
}

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
