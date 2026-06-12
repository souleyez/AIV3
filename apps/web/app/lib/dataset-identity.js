export function buildAutoDatasetIdentity(existingCount = 0, options = {}) {
  const now = options.now instanceof Date ? options.now : new Date();
  const random = typeof options.random === 'function' ? options.random : Math.random;
  const safeStamp = Number.isFinite(now.getTime()) ? now.getTime().toString(36) : String(Date.now());
  const randomPart = random().toString(36).slice(2, 7);
  const titleTime = now.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
  return {
    key: `dataset-${safeStamp}-${randomPart}`,
    title: `新数据集 ${existingCount + 1} · ${titleTime}`,
  };
}
