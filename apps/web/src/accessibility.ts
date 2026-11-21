// The accessibility panel's settings: kept in this device's localStorage and applied to
// <html> as a font size and `a11y-*` classes, so the HTML stays the same for every visitor.

export const switches = ['contrast', 'solid', 'underline', 'spacing'] as const;
export type Switch = (typeof switches)[number];
export type Settings = { size?: TextSize } & { [key in Switch]?: boolean };

export const textSizes = ['87.5%', '100%', '112.5%', '125%'] as const;
export type TextSize = (typeof textSizes)[number];

const key = 'a11y';

// Runs inline in <head> before first paint, allowed by its CSP hash; it must apply what
// `apply` does. Plain ES5 and no module scope, as it runs before any bundle.
export const applySavedSettings = `try{var a=JSON.parse(localStorage.getItem("${key}")||"{}"),h=document.documentElement;if(a.size)h.style.fontSize=a.size;${JSON.stringify(switches)}.forEach(function(k){if(a[k])h.classList.add("a11y-"+k)})}catch(e){}`;

export function load(): Settings {
  try {
    return JSON.parse(localStorage.getItem(key) ?? '{}');
  } catch {
    return {};
  }
}

type Root = {
  style: { fontSize: string };
  classList: { toggle(token: string, force: boolean): unknown };
};

export function apply(settings: Settings, root: Root = document.documentElement) {
  root.style.fontSize = settings.size ?? '';
  for (const name of switches) root.classList.toggle(`a11y-${name}`, !!settings[name]);
}

export function save(settings: Settings) {
  apply(settings);
  try {
    localStorage.setItem(key, JSON.stringify(settings));
  } catch {
    // Private mode or a full quota: the setting still holds until the page is left.
  }
}
