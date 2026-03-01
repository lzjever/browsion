import { describe, it, expect } from 'vitest';
import { formatBytes, profileMatchesFilter } from '../utils';

describe('formatBytes', () => {
  it('should format bytes as B for values < 1024', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1)).toBe('1 B');
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1023)).toBe('1023 B');
  });

  it('should format bytes as KB for values >= 1024 and < 1024*1024', () => {
    expect(formatBytes(1024)).toBe('1.0 KB');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(10240)).toBe('10.0 KB');
    expect(formatBytes(1024 * 1024 - 1)).toBe('1024.0 KB');
  });

  it('should format bytes as MB for values >= 1024*1024', () => {
    expect(formatBytes(1024 * 1024)).toBe('1.0 MB');
    expect(formatBytes(1024 * 1024 * 1.5)).toBe('1.5 MB');
    expect(formatBytes(1024 * 1024 * 10)).toBe('10.0 MB');
  });
});

describe('profileMatchesFilter', () => {
  const mockProfile = {
    name: 'Test Profile',
    tags: ['work', 'development', 'test'],
  };

  it('should return true when filter is empty', () => {
    expect(profileMatchesFilter(mockProfile, '')).toBe(true);
    expect(profileMatchesFilter(mockProfile, '   ')).toBe(true);
  });

  it('should match profile name case-insensitively', () => {
    expect(profileMatchesFilter(mockProfile, 'test')).toBe(true);
    expect(profileMatchesFilter(mockProfile, 'TEST')).toBe(true);
    expect(profileMatchesFilter(mockProfile, 'Test')).toBe(true);
    expect(profileMatchesFilter(mockProfile, 'profile')).toBe(true);
  });

  it('should match profile tags case-insensitively', () => {
    expect(profileMatchesFilter(mockProfile, 'work')).toBe(true);
    expect(profileMatchesFilter(mockProfile, 'WORK')).toBe(true);
    expect(profileMatchesFilter(mockProfile, 'development')).toBe(true);
    expect(profileMatchesFilter(mockProfile, 'test')).toBe(true);
  });

  it('should match any keyword when multiple keywords are provided', () => {
    expect(profileMatchesFilter(mockProfile, 'test profile')).toBe(true);
    expect(profileMatchesFilter(mockProfile, 'work dev')).toBe(true);
    expect(profileMatchesFilter(mockProfile, 'production test')).toBe(true);
  });

  it('should return false when no match found', () => {
    expect(profileMatchesFilter(mockProfile, 'production')).toBe(false);
    expect(profileMatchesFilter(mockProfile, 'staging deploy')).toBe(false);
  });

  it('should handle profiles without tags', () => {
    const profileWithoutTags = {
      name: 'Simple Profile',
    };
    expect(profileMatchesFilter(profileWithoutTags, 'simple')).toBe(true);
    expect(profileMatchesFilter(profileWithoutTags, 'work')).toBe(false);
  });

  it('should handle profiles with empty tags array', () => {
    const profileWithEmptyTags = {
      name: 'Tagless Profile',
      tags: [],
    };
    expect(profileMatchesFilter(profileWithEmptyTags, 'tagless')).toBe(true);
    expect(profileMatchesFilter(profileWithEmptyTags, 'work')).toBe(false);
  });
});
