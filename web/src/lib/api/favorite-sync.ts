import { apiClient } from './client'

export type FavoriteSyncStatus =
  | 'disabled'
  | 'checking'
  | 'needsResolution'
  | 'syncing'
  | 'synced'
  | 'error'

export type FavoriteSyncPendingKind = 'check' | 'merge' | 'remoteOverwrite' | 'setFavorite'

export type FavoriteSyncProgressPhase = 'fetchingRemote' | 'uploadingLocal' | 'verifying'

export type FavoriteSyncResolution = 'merge' | 'remoteOverwrite'

export type FavoriteSyncState = {
  enabled: boolean
  status: FavoriteSyncStatus
  accountUsername: string | null
  localCount: number
  remoteCount: number
  localOnlyCount: number
  remoteOnlyCount: number
  progressDone: number
  progressTotal: number
  progressPhase: FavoriteSyncProgressPhase | null
  pendingKind: FavoriteSyncPendingKind | null
  pendingComicId: string | null
  pendingTarget: boolean | null
  lastError: string | null
  lastCheckedAt: number | null
  lastSyncedAt: number | null
}

export function getFavoriteSyncState(): Promise<FavoriteSyncState> {
  return apiClient.get('/api/settings/favorite-sync')
}

export function setFavoriteSyncEnabled(enabled: boolean): Promise<FavoriteSyncState> {
  return apiClient.put('/api/settings/favorite-sync', { enabled })
}

export function checkFavoriteSync(): Promise<FavoriteSyncState> {
  return apiClient.post('/api/settings/favorite-sync/check')
}

export function resolveFavoriteSync(strategy: FavoriteSyncResolution): Promise<FavoriteSyncState> {
  return apiClient.post('/api/settings/favorite-sync/resolve', { strategy })
}

export function retryFavoriteSync(): Promise<FavoriteSyncState> {
  return apiClient.post('/api/settings/favorite-sync/retry')
}
