import { describe, expect, test } from 'bun:test'

import { mapComicCanonicalId } from './comic'

describe('comic detail API compatibility', () => {
  test('uses an explicit canonical comic id', () => {
    expect(mapComicCanonicalId('1459963', ' 1423951 ')).toBe('1423951')
  })

  test('falls back to the response id when canonicalId is unavailable', () => {
    expect(mapComicCanonicalId('1459963')).toBe('1459963')
    expect(mapComicCanonicalId('1459963', null)).toBe('1459963')
    expect(mapComicCanonicalId('1459963', '   ')).toBe('1459963')
  })
})
