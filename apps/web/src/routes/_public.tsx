import { createFileRoute, getRouteApi, Outlet } from '@tanstack/react-router';
import { regencyShort, villageTitle } from '../village';

const root = getRouteApi('__root__');

export const Route = createFileRoute('/_public')({
  component: PublicLayout,
});

function PublicLayout() {
  const village = root.useLoaderData();
  return (
    <>
      {village && (
        <header>
          <strong>{villageTitle(village)}</strong>
          <small>
            {regencyShort(village)}, {village.province}
          </small>
        </header>
      )}
      <Outlet />
      <footer>
        {village && (
          <>
            <strong>Pemerintah {villageTitle(village)}</strong>
            <p>
              Kec. {village.district}, {regencyShort(village)}, {village.province}
            </p>
          </>
        )}
        <p>Dikelola dengan Pranata</p>
      </footer>
    </>
  );
}
