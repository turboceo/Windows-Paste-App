/**
 * 把 RFC3339 时间转成卡片头部用的相对时间。
 *
 * 卡片上要的是「4 小时前」这种一眼可扫的量级，而不是精确到分钟的时间戳；
 * 精确时间放在 title 里，鼠标悬停仍能看到。
 *
 * 解析失败返回空串——调用方据此隐藏这一行，而不是显示 "Invalid Date"。
 */
export function relativeTime(iso: string): string {
  if (!iso) return ''
  const then = new Date(iso)
  const timestamp = then.getTime()
  if (Number.isNaN(timestamp)) return ''

  const seconds = Math.round((Date.now() - timestamp) / 1000)

  // 时钟回拨、或本机时间比记录时间还早：不显示负数
  if (seconds < 60) return '刚刚'

  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes} 分钟前`

  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} 小时前`

  const days = Math.floor(hours / 24)
  if (days < 7) return `${days} 天前`

  const weeks = Math.floor(days / 7)
  if (days < 30) return `${weeks} 周前`

  // 超过一个月就不再说「N 周前」了，直接给日期更好用
  const month = then.getMonth() + 1
  const day = then.getDate()
  if (then.getFullYear() === new Date().getFullYear()) {
    return `${month} 月 ${day} 日`
  }
  return `${then.getFullYear()} 年 ${month} 月 ${day} 日`
}
