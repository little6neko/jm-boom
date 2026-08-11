import { describe, expect, test } from 'bun:test'

import type { ComicChapter } from '@/domain/comic'
import { getComicChapterPresentation, sortComicChapters } from './comic'

const chapters: ComicChapter[] = [
  { id: '30', title: '', sort: '3' },
  { id: '20', title: '原标题', sort: '2' },
  { id: '10', title: '', sort: '1' }
]

describe('comic chapter presentation', () => {
  test('keeps an original title and adds a compact episode label', () => {
    expect(getComicChapterPresentation(chapters[1], chapters)).toEqual({
      episodeNumber: '2',
      episodeLabel: '第2话',
      hasOriginalTitle: true,
      title: '原标题'
    })
  })

  test('uses the episode label as the title when the original title is empty', () => {
    expect(getComicChapterPresentation(chapters[0], chapters)).toEqual({
      episodeNumber: '3',
      episodeLabel: '第3话',
      hasOriginalTitle: false,
      title: '第3话'
    })
  })

  test('uses the stable ascending position when sort is missing', () => {
    const chaptersWithoutSort: ComicChapter[] = [
      { id: 'newest', title: '', sort: '' },
      { id: 'middle', title: '', sort: '' },
      { id: 'oldest', title: '', sort: '' }
    ]
    const canonicalChapters = sortComicChapters(chaptersWithoutSort)

    expect(getComicChapterPresentation(canonicalChapters[0], canonicalChapters).title).toBe('第3话')
    expect(getComicChapterPresentation(canonicalChapters[1], canonicalChapters).title).toBe('第2话')
    expect(getComicChapterPresentation(canonicalChapters[2], canonicalChapters).title).toBe('第1话')
  })
})
