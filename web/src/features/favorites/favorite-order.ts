import type { FavoriteSyncState } from '@/lib/api/favorite-sync'
import type { FavoriteOrder } from '@/lib/api/favorite'
import { ApiError } from '@/lib/api/client'

export type FavoriteOrderErrorCode =
  | 'favorite_order_unavailable'
  | 'favorite_sync_mismatch'
  | 'favorite_order_stale'

const FAVORITE_ORDER_ERROR_CODES = new Set<FavoriteOrderErrorCode>([
  'favorite_order_unavailable',
  'favorite_sync_mismatch',
  'favorite_order_stale'
])

export function parseFavoriteOrder(value: unknown): FavoriteOrder {
  return value === 'mp' ? 'mp' : 'mr'
}

export function isFavoriteSyncReady(state: FavoriteSyncState | undefined) {
  return state?.enabled === true && state.status === 'synced'
}

export function favoriteOrderOptions(syncReady: boolean) {
  return [
    { label: '收藏时间最新', value: 'mr' },
    {
      label: '更新时间最新',
      value: 'mp',
      disabled: !syncReady,
      description: syncReady ? undefined : '登录并完成收藏同步后可用'
    }
  ] as const
}

export function favoriteOrderErrorCode(error: unknown): FavoriteOrderErrorCode | undefined {
  if (!(error instanceof ApiError) || !isRecord(error.data)) return undefined
  const code = error.data.code
  return typeof code === 'string' && FAVORITE_ORDER_ERROR_CODES.has(code as FavoriteOrderErrorCode)
    ? (code as FavoriteOrderErrorCode)
    : undefined
}

export function shouldRetryFavoriteOrder(failureCount: number, error: unknown) {
  return failureCount < 1 && favoriteOrderErrorCode(error) === 'favorite_order_stale'
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}
