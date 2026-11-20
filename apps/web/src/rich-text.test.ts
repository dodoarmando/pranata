import { describe, expect, it } from '@rstest/core';
import { youtubeId } from './rich-text';

describe('youtubeId', () => {
  it('finds the id in every common YouTube URL and in a bare id', () => {
    for (const input of [
      'dQw4w9WgXcQ',
      'https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42',
      'https://m.youtube.com/watch?v=dQw4w9WgXcQ',
      ' https://youtu.be/dQw4w9WgXcQ?si=abc ',
      'https://www.youtube.com/shorts/dQw4w9WgXcQ',
      'https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ',
    ]) {
      expect(youtubeId(input)).toBe('dQw4w9WgXcQ');
    }
  });

  it('refuses anything else', () => {
    for (const input of [
      '',
      'https://vimeo.com/123456789',
      'https://youtube.com.evil.example/watch?v=dQw4w9WgXcQ',
      'https://www.youtube.com/watch?v=short',
      'javascript:alert(1)',
    ]) {
      expect(youtubeId(input)).toBeNull();
    }
  });
});
