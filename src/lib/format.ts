/** Human byte label: `B` under 1KB, one-decimal `KB` at/above. */
export function formatBytes(bytes: number): string {
  return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
}
