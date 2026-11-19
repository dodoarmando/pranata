// First match wins: Edge and Opera also claim Chrome, Chrome claims Safari, iOS claims Mac OS X,
// and Android claims Linux.
const browsers: [pattern: RegExp, name: string][] = [
  [/Edg(e|A|iOS)?\//, 'Edge'],
  [/OPR\//, 'Opera'],
  [/Firefox\/|FxiOS\//, 'Firefox'],
  [/Chrome\/|CriOS\//, 'Chrome'],
  [/Safari\//, 'Safari'],
];
const systems: [pattern: RegExp, name: string][] = [
  [/Android/, 'Android'],
  [/iPhone|iPad/, 'iOS'],
  [/Windows/, 'Windows'],
  [/Mac OS X/, 'macOS'],
  [/CrOS/, 'ChromeOS'],
  [/Linux/, 'Linux'],
];

const find = (list: [RegExp, string][], userAgent: string) =>
  list.find(([pattern]) => pattern.test(userAgent))?.[1];

/** A session's browser and device in words, such as "Chrome di Android". */
export function describeDevice(userAgent: string | null | undefined): string {
  const browser = find(browsers, userAgent ?? '');
  const system = find(systems, userAgent ?? '');
  if (browser && system) return `${browser} di ${system}`;
  return browser ?? system ?? 'Perangkat tidak dikenal';
}
