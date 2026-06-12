export function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B'
  }
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  const formatted = value >= 10 || Number.isInteger(value) ? value.toFixed(0) : value.toFixed(1)
  return `${formatted} ${units[unit]}`
}

export function formatEpoch(epochSec?: number | null) {
  if (!epochSec) {
    return '-'
  }
  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(epochSec * 1000))
}

export function formatPercent(value: number, total: number) {
  if (!total) {
    return '0%'
  }
  return `${Math.round((value / total) * 100)}%`
}

export function formatDuration(seconds: number) {
  if (!seconds) {
    return '0s'
  }
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const rest = seconds % 60
  if (hours > 0) {
    return `${hours}h ${minutes}m`
  }
  if (minutes > 0) {
    return `${minutes}m ${rest}s`
  }
  return `${rest}s`
}
