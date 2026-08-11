import { describe, expect, test } from 'bun:test'

import type { ComicChapter, ComicDetail } from '@/domain/comic'
import type { ReadingHistoryItem } from '@/lib/api/history'
import { sortComicChapters } from '@/lib/comic'
import { getComicReadingActionLabel, resolveComicReadingTarget } from './reading-target'

describe('comic reading action', () => {
  test('continues a multi-chapter comic from its sort number', () => {
    const chapters = [chapter('30', '3'), chapter('20', '2'), chapter('10', '1')]
    const target = resolveComicReadingTarget(
      comic(chapters),
      sortComicChapters(chapters),
      history('20')
    )

    expect(target.episodeNumber).toBe('2')
    expect(getComicReadingActionLabel(target)).toBe('从第2话继续')
  })

  test('uses the stable ascending position when the history chapter has no sort', () => {
    const chapters = [chapter('newest', ''), chapter('middle', ''), chapter('oldest', '')]
    const target = resolveComicReadingTarget(
      comic(chapters),
      sortComicChapters(chapters),
      history('oldest')
    )

    expect(target.episodeNumber).toBe('1')
    expect(getComicReadingActionLabel(target)).toBe('从第1话继续')
  })

  test('keeps the generic continue label for a single-chapter comic', () => {
    const chapterList = [chapter('10', '1')]
    const target = resolveComicReadingTarget(
      comic(chapterList),
      sortComicChapters(chapterList),
      history('10')
    )

    expect(target.episodeNumber).toBeUndefined()
    expect(getComicReadingActionLabel(target)).toBe('继续阅读')
  })

  test('starts reading when history is missing or no longer belongs to the comic', () => {
    const chapters = [chapter('20', '2'), chapter('10', '1')]
    const sortedChapters = sortComicChapters(chapters)

    expect(
      getComicReadingActionLabel(
        resolveComicReadingTarget(comic(chapters), sortedChapters, undefined)
      )
    ).toBe('开始阅读')
    expect(
      getComicReadingActionLabel(
        resolveComicReadingTarget(comic(chapters), sortedChapters, history('404'))
      )
    ).toBe('开始阅读')
  })
})

function chapter(id: string, sort: string): ComicChapter {
  return { id, sort, title: '' }
}

function comic(chapters: ComicChapter[]): ComicDetail {
  return {
    id: '100',
    title: 'Title',
    description: '',
    image: '',
    tags: [],
    updatedAt: null,
    authors: [],
    actors: [],
    works: [],
    totalViews: 0,
    likes: 0,
    commentCount: 0,
    relatedComics: [],
    chapters
  }
}

function history(chapterId: string): ReadingHistoryItem {
  return {
    id: '100',
    title: 'Title',
    author: '',
    image: '',
    chapterId,
    chapterTitle: '',
    pageIndex: 0,
    pageCount: 10,
    lastReadAt: 1
  }
}
