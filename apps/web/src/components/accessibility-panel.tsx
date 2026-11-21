import { Accessibility, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { load, type Settings, type Switch, save, switches, textSizes } from '../accessibility';

const sizeLabels = {
  '87.5%': ['A−', 'Teks kecil', 'text-[0.8125rem]'],
  '100%': ['A', 'Teks normal', 'text-[0.9375rem]'],
  '112.5%': ['A+', 'Teks besar', 'text-[1.0625rem]'],
  '125%': ['A++', 'Teks sangat besar', 'text-[1.1875rem]'],
} as const satisfies Record<(typeof textSizes)[number], readonly [string, string, string]>;

const switchLabels = {
  contrast: ['Kontras tinggi', 'Teks lebih tegas, latar polos'],
  solid: ['Matikan efek kaca', 'Permukaan solid, tanpa transparansi'],
  underline: ['Garis bawahi tautan', 'Tautan lebih mudah dikenali'],
  spacing: ['Jarak teks lebih lega', 'Baris dan huruf lebih renggang'],
} as const satisfies Record<Switch, readonly [string, string]>;

// A tab on the right edge that slides the panel out; the head script has already applied
// the saved settings, so this only reads them to show their state.
export function AccessibilityPanel() {
  const [open, setOpen] = useState(false);
  const [settings, setSettings] = useState<Settings>({});
  const tab = useRef<HTMLButtonElement>(null);
  const close = useRef<HTMLButtonElement>(null);

  useEffect(() => setSettings(load()), []);
  useEffect(() => {
    if (!open) return;
    close.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      setOpen(false);
      tab.current?.focus();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [open]);

  const change = (next: Settings) => {
    setSettings(next);
    save(next);
  };
  const size = settings.size ?? '100%';

  return (
    <div
      className={`fixed top-1/2 right-0 z-60 flex items-start transition-transform duration-250 motion-reduce:transition-none ${open ? '-translate-y-1/2' : '-translate-y-1/2 translate-x-[calc(100%-2.75rem)]'}`}
    >
      <button
        ref={tab}
        type="button"
        aria-expanded={open}
        aria-controls="a11y-panel"
        aria-label={open ? 'Tutup pengaturan aksesibilitas' : 'Buka pengaturan aksesibilitas'}
        onClick={() => setOpen(!open)}
        className="grid size-11 flex-none place-items-center rounded-l-md bg-primary text-primary-foreground shadow-lift-dark hover:bg-pri-d"
      >
        <Accessibility className="size-6" aria-hidden="true" />
      </button>
      <section
        id="a11y-panel"
        aria-labelledby="a11y-title"
        inert={!open}
        className="glass grid max-h-[calc(100dvh-2rem)] w-[min(20rem,100vw-3.5rem)] gap-5 overflow-y-auto rounded-none rounded-bl-md border-r-0 p-5"
      >
        <div className="flex items-center justify-between">
          <h2 id="a11y-title" className="text-[1.0625rem] font-semibold">
            Aksesibilitas
          </h2>
          <button
            ref={close}
            type="button"
            aria-label="Tutup pengaturan aksesibilitas"
            onClick={() => {
              setOpen(false);
              tab.current?.focus();
            }}
            className="grid size-9 place-items-center rounded-md hover:bg-muted"
          >
            <X className="size-5" aria-hidden="true" />
          </button>
        </div>
        <fieldset className="grid gap-2">
          <legend className="mb-2 text-[0.8125rem] font-semibold text-muted-foreground">
            Ukuran teks
          </legend>
          <div className="grid grid-cols-4 gap-2">
            {textSizes.map((value) => {
              const [text, label, className] = sizeLabels[value];
              return (
                <button
                  key={value}
                  type="button"
                  aria-pressed={size === value}
                  aria-label={label}
                  onClick={() => {
                    const { size: _, ...rest } = settings;
                    change(value === '100%' ? rest : { ...rest, size: value });
                  }}
                  className={`${className} rounded-md border bg-white py-2 font-semibold aria-pressed:border-primary aria-pressed:bg-primary aria-pressed:text-primary-foreground`}
                >
                  {text}
                </button>
              );
            })}
          </div>
        </fieldset>
        <fieldset className="grid gap-2">
          <legend className="mb-2 text-[0.8125rem] font-semibold text-muted-foreground">
            Tampilan
          </legend>
          {switches.map((name) => {
            const [title, hint] = switchLabels[name];
            const on = !!settings[name];
            return (
              <button
                key={name}
                type="button"
                role="switch"
                aria-checked={on}
                onClick={() => change({ ...settings, [name]: !on })}
                className="group flex w-full items-center justify-between gap-4 rounded-md border bg-white px-3 py-2.5 text-left aria-checked:border-primary"
              >
                <span>
                  <b className="block text-[0.9375rem] font-medium">{title}</b>
                  <small className="block text-xs text-muted-foreground">{hint}</small>
                </span>
                <i
                  aria-hidden="true"
                  className="relative h-5 w-9 flex-none rounded-[3px] bg-[#bdbdbd] transition-colors group-aria-checked:bg-primary after:absolute after:top-0.75 after:left-0.75 after:size-3.5 after:rounded-[2px] after:bg-white after:transition-transform group-aria-checked:after:translate-x-4"
                />
              </button>
            );
          })}
        </fieldset>
        <div className="flex items-center justify-between gap-3 text-xs text-muted-foreground">
          <span>Pengaturan tersimpan di perangkat ini.</span>
          <button
            type="button"
            onClick={() => change({})}
            className="text-sm font-medium whitespace-nowrap text-primary hover:underline"
          >
            Atur ulang
          </button>
        </div>
      </section>
    </div>
  );
}
