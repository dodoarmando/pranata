import { getRouteApi } from '@tanstack/react-router';
import { ArrowLeft } from 'lucide-react';
import type { ReactNode } from 'react';
import { villageInitial, villageTitle } from '../village';

const root = getRouteApi('__root__');

/** The sign-in look: a village scene beside a centred card. */
export function AuthLayout({ title, children }: { title: string; children: ReactNode }) {
  const village = root.useLoaderData();
  return (
    <div className="flex min-h-dvh">
      <div className="hidden flex-1 p-4 lg:flex" aria-hidden="true">
        <div className="relative flex-1 overflow-hidden rounded-lg bg-linear-to-b from-[#bfdce4] to-[#e8e3c6]">
          <svg
            className="absolute inset-0 size-full"
            viewBox="0 0 688 868"
            preserveAspectRatio="xMidYMax slice"
            aria-hidden="true"
          >
            <circle cx="591" cy="234" r="95" fill="#f6d27a" opacity=".9" />
            <path
              d="M0 477.4 C137.6 364.6 261.4 434 378.4 399.3 S584.8 364.6 688 434 L688 868 L0 868Z"
              fill="#7fae7a"
            />
            <path
              d="M0 572.9 C137.6 486.1 261.4 538.2 378.4 503.4 S584.8 486.1 688 538.2 L688 868 L0 868Z"
              fill="#4e8a5c"
            />
            <path
              d="M0 694.4 C137.6 625 261.4 668.4 378.4 607.6 S584.8 625 688 668.4 L688 868 L0 868Z"
              fill="#2d6346"
            />
          </svg>
          <div className="absolute inset-0 bg-linear-to-b from-pri-dd/50 via-transparent to-pri-dd/70" />
        </div>
      </div>
      <main className="flex flex-1 flex-col items-center justify-between gap-10 px-5 py-6 md:p-8 lg:px-10">
        <span />
        <div className="flex w-full max-w-110 flex-col gap-10">
          <header>
            <span className="mb-3 inline-flex rounded-md bg-muted p-2">
              <span
                className="grid size-8 place-items-center rounded-full bg-primary text-xs font-bold text-primary-foreground"
                aria-hidden="true"
              >
                {villageInitial(village)}
              </span>
            </span>
            <h1 className="mb-1 text-[1.625rem] leading-tight font-medium tracking-tight md:text-3xl">
              {title}
            </h1>
            {village && <p className="text-muted-foreground">Pemerintah {villageTitle(village)}</p>}
          </header>
          {children}
        </div>
        <footer className="flex flex-col items-center gap-3 text-center">
          <a
            href="/"
            className="inline-flex items-center gap-2 text-sm font-medium text-primary hover:underline"
          >
            <ArrowLeft className="size-4" aria-hidden="true" />
            Kembali ke situs desa
          </a>
          <small className="text-xs text-muted-foreground">Dikelola dengan Pranata</small>
        </footer>
      </main>
    </div>
  );
}

/** The sign-in look's text field classes: 16px text stops iOS zooming in on focus. */
export const authInput = 'h-12 px-3.5 text-base md:text-base';
export const authSubmit = 'h-12 text-[0.9375rem]';
