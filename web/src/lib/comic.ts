import type { ComicChapter } from '@/domain/comic'

export const SINGLE_CHAPTER_TITLE = '单章'

export type ComicChapterPresentation = {
  episodeNumber: string
  episodeLabel: string
  hasOriginalTitle: boolean
  title: string
}

export function sortComicChapters(chapters: ComicChapter[]) {
  return [...chapters].sort((left, right) => {
    const leftSort = Number.parseInt(left.sort, 10)
    const rightSort = Number.parseInt(right.sort, 10)

    if (Number.isNaN(leftSort) || Number.isNaN(rightSort)) {
      return 0
    }

    return rightSort - leftSort
  })
}

export function getComicChapterPresentation(
  chapter: ComicChapter,
  chapters: ComicChapter[]
): ComicChapterPresentation {
  const title = chapter.title.trim()
  const episodeNumber = chapter.sort.trim() || getFallbackEpisodeNumber(chapter, chapters)
  const episodeLabel = `第${episodeNumber}话`

  return {
    episodeNumber,
    episodeLabel,
    hasOriginalTitle: title.length > 0,
    title: title || episodeLabel
  }
}

export function formatComicChapterTitle(chapter: ComicChapter, chapters: ComicChapter[]) {
  return getComicChapterPresentation(chapter, chapters).title
}

export function getComicDisplayChapterCount(chapters: ComicChapter[]) {
  return Math.max(chapters.length, 1)
}

export function mergeReadChapterIds(readChapterIds: string[], chapterId: string) {
  if (!chapterId || readChapterIds.includes(chapterId)) {
    return readChapterIds
  }

  return [...readChapterIds, chapterId]
}

export function getReadChapterToneClassName({
  isRead,
  isCurrent = false
}: {
  isRead: boolean
  isCurrent?: boolean
}) {
  return isRead && !isCurrent
    ? 'bg-muted text-muted-foreground hover:bg-muted/70 hover:text-foreground'
    : undefined
}

function getFallbackEpisodeNumber(chapter: ComicChapter, chapters: ComicChapter[]) {
  const descendingChapters = sortComicChapters(chapters)
  const descendingIndex = descendingChapters.findIndex(candidate => candidate.id === chapter.id)

  if (descendingIndex < 0) {
    return '1'
  }

  return String(descendingChapters.length - descendingIndex)
}
