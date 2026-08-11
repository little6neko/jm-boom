import { describe, expect, test } from 'bun:test'

import { resolveCanonicalComicRedirect } from './canonical-redirect'

describe('canonical comic redirects', () => {
  test('redirects a chapter alias to its canonical comic', () => {
    expect(resolveCanonicalComicRedirect('1459963', '1423951')).toBe('1423951')
    expect(resolveCanonicalComicRedirect('1459963', ' 1423951 ')).toBe('1423951')
  })

  test('does not redirect an already canonical comic', () => {
    expect(resolveCanonicalComicRedirect('1423951', '1423951')).toBeNull()
  })

  test('does not redirect when the canonical id is missing or invalid', () => {
    for (const canonicalId of [undefined, null, '', '0', '000', 'not-an-id', '12.3']) {
      expect(resolveCanonicalComicRedirect('1459963', canonicalId)).toBeNull()
    }
  })
})
