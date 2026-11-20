import { Link } from '@tanstack/react-router';

/** A public 404: what is missing, and the way on to the type's list, to search and home. */
export function NotFound({
  title,
  text,
  list,
}: {
  title: string;
  text: string;
  list?: { to: '/berita' | '/pengumuman' | '/galeri'; label: string };
}) {
  const link = 'font-medium text-primary';
  return (
    <main className="site-container grid gap-4 pt-10 pb-sec md:pt-16">
      <h1 className="text-[2rem] leading-tight font-semibold tracking-tight">{title}</h1>
      <p className="text-muted-foreground">{text}</p>
      <p className="flex flex-wrap gap-4">
        {list && (
          <Link to={list.to} className={link}>
            {list.label}
          </Link>
        )}
        <Link to="/cari" className={link}>
          Cari di situs ini
        </Link>
        <Link to="/" className={link}>
          Kembali ke beranda
        </Link>
      </p>
    </main>
  );
}
