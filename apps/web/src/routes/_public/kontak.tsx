import { createFileRoute, getRouteApi } from '@tanstack/react-router';
import { createServerFn } from '@tanstack/react-start';
import { ExternalLink, Mail, MapPin, Phone } from 'lucide-react';
import { listPublishedImportantContacts } from '../../api/generated';
import { pageHead } from '../../seo';
import { regencyShort, villageTitle } from '../../village';

const root = getRouteApi('__root__');

// Server-only so the browser never needs to reach the API directly.
const fetchImportantContacts = createServerFn({ method: 'GET' }).handler(async () => {
  try {
    const response = await listPublishedImportantContacts();
    return response.status === 200 ? response.data : [];
  } catch {
    return [];
  }
});

const telHref = (phone: string) => `tel:${phone.replace(/[^\d+]/g, '')}`;

export const Route = createFileRoute('/_public/kontak')({
  loader: () => fetchImportantContacts(),
  head: (context) => pageHead(context, { title: 'Kontak' }),
  component: ContactPage,
});

function ContactPage() {
  const village = root.useLoaderData();
  const groups = Route.useLoaderData();
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
              <a href={telHref(village.phone)}>{village.phone}</a>
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
      {groups.length > 0 && (
        <section aria-labelledby="important-contacts" className="grid gap-6">
          <h2 id="important-contacts" className="text-2xl font-semibold tracking-tight">
            Kontak penting
          </h2>
          <div className="grid gap-6 md:grid-cols-2 xl:grid-cols-3">
            {groups.map((group) => (
              <section key={group.category} aria-label={group.category} className="grid gap-3">
                <h3 className="font-semibold">{group.category}</h3>
                <ul className="grid gap-3">
                  {group.contacts.map((contact, index) => (
                    // A static list the server orders; a place is unique where names may repeat.
                    // biome-ignore lint/suspicious/noArrayIndexKey: see above
                    <li key={index} className="flex gap-3">
                      <Phone className="mt-1 size-4 flex-none text-primary" aria-hidden="true" />
                      <span>
                        {contact.name}
                        <a href={telHref(contact.phone)} className="block font-medium">
                          {contact.phone}
                        </a>
                        {contact.note && (
                          <span className="block text-sm text-muted-foreground">
                            {contact.note}
                          </span>
                        )}
                      </span>
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </div>
        </section>
      )}
    </main>
  );
}
