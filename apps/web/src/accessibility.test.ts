import { expect, test } from '@rstest/core';
import { apply, applySavedSettings, type Settings } from './accessibility';

function fakeRoot() {
  const classes = new Set<string>();
  return {
    style: { fontSize: '' },
    classList: {
      add: (token: string) => classes.add(token),
      toggle: (token: string, force: boolean) =>
        force ? classes.add(token) : classes.delete(token),
    },
    classes,
  };
}

// What the inline head script does to <html> for what localStorage holds.
function headScript(stored: string | null) {
  const root = fakeRoot();
  const run = new Function('localStorage', 'document', applySavedSettings);
  run({ getItem: () => stored }, { documentElement: root });
  return { fontSize: root.style.fontSize, classes: [...root.classes].sort() };
}

test.each<Settings>([
  {},
  { size: '125%' },
  { contrast: true, underline: true },
  { size: '87.5%', solid: true, spacing: true, contrast: false },
])('the head script applies %j as the panel does', (settings) => {
  const root = fakeRoot();
  apply(settings, root);
  expect(headScript(JSON.stringify(settings))).toEqual({
    fontSize: root.style.fontSize,
    classes: [...root.classes].sort(),
  });
});

test('the head script ignores what it cannot read', () => {
  expect(headScript('{not json')).toEqual({ fontSize: '', classes: [] });
  expect(headScript(null)).toEqual({ fontSize: '', classes: [] });
});
