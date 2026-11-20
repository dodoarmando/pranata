import { describe, expect, it } from '@rstest/core';
import { moved } from './order';

describe('moved', () => {
  it('moves one item up, down or across, leaving the list as it was', () => {
    const list = ['a', 'b', 'c', 'd'];
    expect(moved(list, 1, 0)).toEqual(['b', 'a', 'c', 'd']);
    expect(moved(list, 1, 2)).toEqual(['a', 'c', 'b', 'd']);
    expect(moved(list, 3, 0)).toEqual(['d', 'a', 'b', 'c']);
    expect(list).toEqual(['a', 'b', 'c', 'd']);
  });
});
