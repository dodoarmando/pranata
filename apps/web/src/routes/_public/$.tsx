import { createFileRoute, notFound } from '@tanstack/react-router';
import { NotFound } from '../../components/not-found';

/** Any address no other route takes, answered 404 inside the public layout. */
export const Route = createFileRoute('/_public/$')({
  loader: () => {
    throw notFound();
  },
  notFoundComponent: () => (
    <NotFound title="Halaman tidak ditemukan" text="Alamat ini tidak ada di situs ini." />
  ),
});
