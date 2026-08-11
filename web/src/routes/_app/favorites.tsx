import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ListFilterIcon, LoaderCircleIcon, Trash2Icon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'

import { AppPage } from '@/components/app-page'
import { ComicGrid } from '@/components/comic'
import { ConfirmDialog } from '@/components/confirm-dialog'
import { EmptyState } from '@/components/empty-state'
import { FilterSelect } from '@/components/filter-select'
import { useFavoriteSyncState } from '@/features/favorite-sync/use-favorite-sync-state'
import {
  favoriteOrderErrorCode,
  favoriteOrderOptions,
  isFavoriteSyncReady,
  parseFavoriteOrder,
  shouldRetryFavoriteOrder
} from '@/features/favorites/favorite-order'
import { ListPagination } from '@/components/list-pagination'
import { PageHeader } from '@/components/page-header'
import { Button } from '@/components/ui/button'
import type { ComicStateResult } from '@/lib/api/comic'
import { clearFavorites, listFavorites, type FavoriteOrder } from '@/lib/api/favorite'
import { UI } from '@/lib/constants'
import { queryKeys } from '@/lib/query-keys'

type FavoritesSearch = {
  order: FavoriteOrder
}

export const Route = createFileRoute('/_app/favorites')({
  validateSearch: (search: Record<string, unknown>): FavoritesSearch => ({
    order: parseFavoriteOrder(search.order)
  }),
  component: FavoritesPage
})

function FavoritesPage() {
  const navigate = useNavigate({ from: Route.fullPath })
  const search = Route.useSearch()
  const queryClient = useQueryClient()
  const favoriteSync = useFavoriteSyncState()
  const syncReady = isFavoriteSyncReady(favoriteSync.data)
  const previousSyncReady = useRef<boolean | undefined>(undefined)
  const [page, setPage] = useState(1)
  const query = useQuery({
    queryKey: queryKeys.favorites(page, search.order),
    queryFn: () => listFavorites(page, search.order),
    enabled: search.order === 'mr' || syncReady,
    staleTime: 0,
    retry: shouldRetryFavoriteOrder
  })
  const orderErrorCode = favoriteOrderErrorCode(query.error)
  const shouldFallBackOrder =
    search.order === 'mp' &&
    (orderErrorCode === 'favorite_order_unavailable' || (!favoriteSync.isPending && !syncReady))

  useEffect(() => {
    const rawOrder = new URLSearchParams(window.location.search).get('order')
    if (rawOrder === search.order) return
    void navigate({
      replace: true,
      resetScroll: false,
      search: { order: search.order }
    })
  }, [navigate, search.order])

  useEffect(() => {
    if (!shouldFallBackOrder) return
    setPage(1)
    void navigate({
      replace: true,
      resetScroll: false,
      search: { order: 'mr' }
    })
  }, [navigate, shouldFallBackOrder])

  useEffect(() => {
    if (favoriteSync.data === undefined) return
    const previous = previousSyncReady.current
    previousSyncReady.current = syncReady
    if (previous !== undefined && previous !== syncReady) {
      void queryClient.invalidateQueries({ queryKey: queryKeys.favorites() })
    }
  }, [favoriteSync.data, queryClient, syncReady])

  const { mutate: clear, isPending: isClearing } = useMutation({
    mutationFn: clearFavorites,
    onSuccess: () => {
      setPage(1)
      queryClient.setQueriesData({ queryKey: queryKeys.favorites() }, { items: [], total: 0 })
      queryClient.setQueriesData<ComicStateResult>({ queryKey: ['jm-comic-state'] }, current =>
        current ? { ...current, isFavorite: false } : current
      )
      toast.success('收藏已清空')
    },
    onError: error => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.favoriteSync() })
      toast.error(error instanceof Error ? error.message : '清空收藏失败')
    }
  })
  const items = query.data?.items ?? []
  const total = query.data?.total ?? 0
  const pageCount = Math.ceil(total / UI.COLLECTION_PAGE_SIZE)
  const isLoading =
    query.isLoading || (search.order === 'mp' && (favoriteSync.isPending || shouldFallBackOrder))
  const errorTitle =
    orderErrorCode === 'favorite_sync_mismatch'
      ? '本地与远端收藏已不一致，请在设置中重新检查同步'
      : orderErrorCode === 'favorite_order_stale'
        ? '收藏顺序已变化，请重试'
        : '收藏加载失败'

  function updateOrder(value: string) {
    const order = parseFavoriteOrder(value)
    if (order === 'mp' && !syncReady) return
    setPage(1)
    void navigate({
      replace: true,
      resetScroll: false,
      search: { order }
    })
  }

  return (
    <AppPage>
      <PageHeader title="收藏" description="服务端收藏的漫画">
        <FilterSelect
          value={search.order}
          options={favoriteOrderOptions(syncReady)}
          placeholder="选择排序"
          icon={<ListFilterIcon className="size-4 text-muted-foreground" />}
          grow={false}
          onValueChange={updateOrder}
        />
        {favoriteSync.data?.enabled !== true ? (
          <ConfirmDialog
            trigger={
              <Button variant="destructive" size="sm" disabled={total === 0 || isClearing}>
                <Trash2Icon className="size-4" />
                清空收藏
              </Button>
            }
            icon={<Trash2Icon className="size-5 text-destructive" />}
            title="清空服务端收藏"
            description="这会删除当前服务端中所有设备共享的收藏记录，操作后无法恢复。"
            confirmText="确认清空"
            variant="destructive"
            loading={isClearing}
            onConfirm={() => clear()}
          />
        ) : null}
      </PageHeader>

      {isLoading ? (
        <div className="flex flex-1 items-center justify-center">
          <LoaderCircleIcon className="size-6 animate-spin text-muted-foreground" />
        </div>
      ) : query.isError ? (
        <EmptyState
          className="min-h-0 flex-1"
          emoji="Ò︵Ó"
          title={errorTitle}
          actions={
            <div className="flex flex-wrap justify-center gap-2">
              {orderErrorCode === 'favorite_sync_mismatch' ? (
                <Button variant="outline" size="sm" asChild>
                  <Link to="/settings">前往设置</Link>
                </Button>
              ) : null}
              <Button variant="outline" size="sm" onClick={() => query.refetch()}>
                重试
              </Button>
            </div>
          }
        />
      ) : items.length === 0 ? (
        <EmptyState className="min-h-0 flex-1" emoji="(･o･;)" title="暂无收藏" />
      ) : (
        <ComicGrid items={items} />
      )}

      {pageCount > 1 ? (
        <ListPagination
          page={page}
          hasMore={page < pageCount}
          disabled={query.isFetching || isClearing}
          onPageChange={setPage}
        />
      ) : null}
    </AppPage>
  )
}
