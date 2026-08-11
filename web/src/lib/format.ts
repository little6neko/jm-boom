const BYTE_UNITS = ['B', 'KB', 'MB', 'GB'] as const
const CHINESE_DATE_TIME_FORMATTER = new Intl.DateTimeFormat('zh-CN', {
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit'
})
const STANDARD_NUMBER_FORMATTER = new Intl.NumberFormat('zh-CN')
const COMPACT_NUMBER_FORMATTER = new Intl.NumberFormat('zh-CN', {
  notation: 'compact',
  maximumFractionDigits: 1
})

export function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B'
  }

  let value = bytes
  let unitIndex = 0

  while (value >= 1024 && unitIndex < BYTE_UNITS.length - 1) {
    value /= 1024
    unitIndex += 1
  }

  return `${value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${BYTE_UNITS[unitIndex]}`
}

export function formatDuration(seconds: number) {
  if (seconds < 60) return `${seconds} 秒`

  const minutes = Math.ceil(seconds / 60)

  if (minutes < 60) return `${minutes} 分钟`

  return `${Math.ceil(minutes / 60)} 小时`
}

export function formatDate(value: number) {
  return CHINESE_DATE_TIME_FORMATTER.format(new Date(value))
}

export function formatUnixDateTime(value: number | null | undefined) {
  if (typeof value !== 'number' || !Number.isFinite(value) || value <= 0) {
    return null
  }

  const date = new Date(value * 1000)
  if (Number.isNaN(date.getTime())) {
    return null
  }

  const pad = (part: number) => String(part).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

export function formatNumber(value: number) {
  return (value >= 10000 ? COMPACT_NUMBER_FORMATTER : STANDARD_NUMBER_FORMATTER).format(value)
}
