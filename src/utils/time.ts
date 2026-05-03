/** Format relative time in Chinese */
export function formatRelativeTime(dateStr: string): string {
  const date = new Date(`${dateStr}Z`); // UTC
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);

  if (diffSec < 10) {
    return "刚刚";
  }
  if (diffSec < 60) {
    return `${diffSec}秒前`;
  }
  if (diffMin < 60) {
    return `${diffMin}分钟前`;
  }
  if (diffHour < 24) {
    return `${diffHour}小时前`;
  }
  if (diffDay < 7) {
    return `${diffDay}天前`;
  }
  if (diffDay < 30) {
    return `${Math.floor(diffDay / 7)}周前`;
  }
  return date.toLocaleDateString("zh-CN");
}

/** Format byte size */
export function formatByteSize(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
