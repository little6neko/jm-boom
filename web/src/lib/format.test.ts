import { describe, expect, test } from 'bun:test'

import { formatUnixDateTime } from './format'

describe('Unix date-time formatting', () => {
  test('formats Unix seconds in the browser local timezone to minutes', () => {
    const localTime = new Date(2026, 7, 10, 22, 50, 42).getTime() / 1000
    expect(formatUnixDateTime(localTime)).toBe('2026-08-10 22:50')
  })

  test('rejects missing and invalid timestamps', () => {
    expect(formatUnixDateTime(null)).toBeNull()
    expect(formatUnixDateTime(undefined)).toBeNull()
    expect(formatUnixDateTime(0)).toBeNull()
    expect(formatUnixDateTime(-1)).toBeNull()
    expect(formatUnixDateTime(Number.NaN)).toBeNull()
    expect(formatUnixDateTime(Number.POSITIVE_INFINITY)).toBeNull()
  })
})
