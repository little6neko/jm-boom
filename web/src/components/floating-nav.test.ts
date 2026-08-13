import { describe, expect, test } from 'bun:test'

import { resolveFloatingNavActiveId, type FloatingNavItem } from '@/components/floating-nav'

const items: Array<Pick<FloatingNavItem, 'id' | 'to'>> = [
  { id: 'bookshelf', to: '/bookshelf' },
  { id: 'explore', to: '/explore' },
  { id: 'favorites', to: '/favorites' },
  { id: 'downloads', to: '/downloads' },
  { id: 'settings', to: '/settings' }
]

describe('floating app navigation state', () => {
  test('highlights a matching main section and its nested routes', () => {
    expect(resolveFloatingNavActiveId('/bookshelf', items)).toBe('bookshelf')
    expect(resolveFloatingNavActiveId('/explore/search', items)).toBe('explore')
  })

  test('does not highlight a main section on comic details or the reader', () => {
    expect(resolveFloatingNavActiveId('/comic/1455765', items)).toBeUndefined()
    expect(resolveFloatingNavActiveId('/reader/1455765', items)).toBeUndefined()
  })
})
