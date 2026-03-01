/**
 * Format bytes as human-readable string (B, KB, MB)
 */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * Check if a profile matches a filter string (searches name and tags)
 */
export function profileMatchesFilter(
  profile: { name: string; tags?: string[] },
  filter: string
): boolean {
  if (!filter.trim()) return true;
  const keywords = filter.trim().toLowerCase().split(/\s+/);
  return keywords.some(
    (kw) =>
      profile.name.toLowerCase().includes(kw) ||
      (profile.tags || []).some((tag) => tag.toLowerCase().includes(kw))
  );
}
