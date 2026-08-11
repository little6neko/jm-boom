import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import type { ComicDetail } from '@/domain/comic'
import { useFavoriteSyncState } from '@/features/favorite-sync/use-favorite-sync-state'
import type { ComicStateResult } from '@/lib/api/comic'
import { addFavorite, removeFavorite } from '@/lib/api/favorite'
import { queryKeys } from '@/lib/query-keys'

export function useComicFavorite({
  comic,
  state,
  stateLoading
}: {
  comic: ComicDetail
  state: ComicStateResult | undefined
  stateLoading: boolean
}) {
  const queryClient = useQueryClient()
  const favoriteSync = useFavoriteSyncState()
  const isFavorite = state?.isFavorite ?? false
  const isSyncBlocked = favoriteSync.data?.enabled === true && favoriteSync.data.status !== 'synced'
  const mutation = useMutation({
    mutationFn: async () => {
      if (isFavorite) {
        await removeFavorite(comic.id)
        return { isFavorite: false as const }
      }

      await addFavorite({
        id: comic.id,
        title: comic.title,
        author: comic.authors.join(' / '),
        description: comic.description,
        image: comic.image,
        tags: comic.tags
      })
      return { isFavorite: true as const }
    },
    onSuccess: result => {
      queryClient.setQueryData<ComicStateResult>(queryKeys.comicState(comic.id), current => ({
        isFavorite: result.isFavorite,
        history: current?.history ?? null,
        readChapterIds: current?.readChapterIds ?? []
      }))
      void queryClient.invalidateQueries({ queryKey: queryKeys.favorites() })
      void queryClient.invalidateQueries({ queryKey: queryKeys.favoriteSync() })
      toast.success(result.isFavorite ? '已添加收藏' : '已取消收藏')
    },
    onError: error => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.favoriteSync() })
      toast.error(error instanceof Error ? error.message : '收藏操作失败')
    }
  })

  function toggle() {
    if (isSyncBlocked) {
      toast.error('请先在设置中完成或重试收藏同步')
      return
    }
    mutation.mutate()
  }

  return {
    isFavorite,
    isPending: stateLoading || mutation.isPending || isSyncBlocked,
    toggle
  }
}
