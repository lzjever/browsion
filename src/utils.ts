/**
 * Format bytes as human-readable string (B, KB, MB)
 */
import type { BrowserProfile, RunningStatus } from './types/profile';

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

export function areRunningStatusesEqual(
  a: RunningStatus,
  b: RunningStatus
): boolean {
  const aKeys = Object.keys(a);
  const bKeys = Object.keys(b);

  if (aKeys.length !== bKeys.length) {
    return false;
  }

  for (const key of aKeys) {
    if (a[key] !== b[key]) {
      return false;
    }
  }

  return true;
}

export function areProfilesEqual(a: BrowserProfile, b: BrowserProfile): boolean {
  if (
    a.id !== b.id ||
    a.name !== b.name ||
    a.description !== b.description ||
    a.user_data_dir !== b.user_data_dir ||
    a.proxy_server !== b.proxy_server ||
    a.lang !== b.lang ||
    a.timezone !== b.timezone ||
    a.fingerprint !== b.fingerprint ||
    a.color !== b.color ||
    a.headless !== b.headless
  ) {
    return false;
  }

  if (a.custom_args.length !== b.custom_args.length || a.tags.length !== b.tags.length) {
    return false;
  }

  for (let i = 0; i < a.custom_args.length; i += 1) {
    if (a.custom_args[i] !== b.custom_args[i]) {
      return false;
    }
  }

  for (let i = 0; i < a.tags.length; i += 1) {
    if (a.tags[i] !== b.tags[i]) {
      return false;
    }
  }

  return true;
}

export function mergeProfilesById(
  previous: BrowserProfile[],
  next: BrowserProfile[]
): BrowserProfile[] {
  const previousById = new Map(previous.map((profile) => [profile.id, profile]));
  let changed = previous.length !== next.length;

  const merged = next.map((profile, index) => {
    const existing = previousById.get(profile.id);
    if (!existing) {
      changed = true;
      return profile;
    }

    if (previous[index]?.id !== profile.id) {
      changed = true;
    }

    if (!areProfilesEqual(existing, profile)) {
      changed = true;
      return profile;
    }

    return existing;
  });

  return changed ? merged : previous;
}
