import { useQuery } from '@tanstack/react-query'

import { getFavoriteSyncState, type FavoriteSyncState } from '@/lib/api/favorite-sync'
import { queryKeys } from '@/lib/query-keys'

export function favoriteSyncRefetchInterval(state: FavoriteSyncState | undefined) {
  return state?.status === 'checking' || state?.status === 'syncing' ? 1000 : false
}

export function useFavoriteSyncState() {
  return useQuery({
    queryKey: queryKeys.favoriteSync(),
    queryFn: getFavoriteSyncState,
    staleTime: 2000,
    refetchInterval: query => favoriteSyncRefetchInterval(query.state.data),
    refetchOnWindowFocus: true
  })
}
