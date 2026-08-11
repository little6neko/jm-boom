import type { ComicSummary } from '@/domain/comic'
import { apiClient } from './client'

export type FavoriteItem = ComicSummary & {
  favoritedAt: number
}

export type FavoriteOrder = 'mr' | 'mp'

export type FavoriteListResult = {
  items: FavoriteItem[]
  total: number
}

export function listFavorites(
  page: number,
  order: FavoriteOrder = 'mr'
): Promise<FavoriteListResult> {
  return apiClient.get('/api/favorites', { page, order })
}

export function addFavorite(comic: ComicSummary): Promise<FavoriteItem> {
  return apiClient.put<FavoriteItem>(`/api/favorites/${comic.id}`, {
    title: comic.title,
    author: comic.author,
    description: comic.description,
    image: comic.image,
    tags: comic.tags
  })
}

export function removeFavorite(comicId: string): Promise<void> {
  return apiClient.delete(`/api/favorites/${comicId}`)
}

export function clearFavorites(): Promise<void> {
  return apiClient.delete('/api/favorites')
}
