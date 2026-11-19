import { createFileRoute, getRouteApi } from '@tanstack/react-router';
import { ExternalLink, Mail, MapPin, Phone } from 'lucide-react';
import type { Village } from '../../api/generated';
import { regencyShort, villageTitle } from '../../village';

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public/kontak')({
  head: ({ matches }) => {
    // The root match comes first; its loader data is typed only through getRouteApi.
    const village = matches[0]?.loaderData as Village | null | undefined;
    return { meta: [{ title: village ? `Kontak — ${villageTitle(village)}` : 'Kontak' }] };
  },
  component: ContactPage,
});

function ContactPage() {
  const village = root.useLoaderData();
  const maps =
    village?.latitude != null && village.longitude != null
      ? `https://www.google.com/maps/search/?api=1&query=${village.latitude},${village.longitude}`
      : null;
  return (
    <main className="site-container grid gap-6 pt-10 pb-sec md:pt-16">
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight md:text-[2.5rem]">
        Kontak
      </h1>
      {village ? (
        <address className="grid max-w-xl gap-4 not-italic md:text-lg">
          <p className="flex gap-3">
            <MapPin className="mt-1 size-5 flex-none text-primary" aria-hidden="true" />
            <span>
              Kantor {villageTitle(village)}
              {village.address && <span className="block">{village.address}</span>}
              <span className="block">
                Kec. {village.district}, {regencyShort(village)}, {village.province}
                {village.postalCode && ` ${village.postalCode}`}
              </span>
              {maps && (
                <a
                  href={maps}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="mt-2 inline-flex items-center gap-1.5 font-medium text-primary underline-offset-4 hover:underline"
                >
                  Buka di Google Maps
                  <ExternalLink className="size-4" aria-hidden="true" />
                </a>
              )}
            </span>
          </p>
          {village.phone && (
            <p className="flex gap-3">
              <Phone className="mt-1 size-5 flex-none text-primary" aria-hidden="true" />
              <a href={`tel:${village.phone.replace(/[^\d+]/g, '')}`}>{village.phone}</a>
            </p>
          )}
          {village.email && (
            <p className="flex gap-3">
              <Mail className="mt-1 size-5 flex-none text-primary" aria-hidden="true" />
              <a href={`mailto:${village.email}`}>{village.email}</a>
            </p>
          )}
        </address>
      ) : (
        <p className="text-muted-foreground">Informasi kontak belum tersedia.</p>
      )}
    </main>
  );
}
