/** Invalid values disable diagnostics rather than turning a bad interval into a hot timer. */
export function memoryLogInterval(value: string | undefined) {
  if (!value || !/^\d+(\.\d+)?$/.test(value)) return undefined
  const seconds = Number(value)

  return seconds >= 1 && seconds <= 2_147_483 ? seconds * 1000 : undefined
}

export function remoteDebuggingPort(packaged: boolean, value: string | undefined) {
  if (value === undefined) return packaged ? undefined : "9222"

  if (!/^\d+$/.test(value)) return undefined
  const port = Number(value)

  return port >= 1 && port <= 65535 ? String(port) : undefined
}
