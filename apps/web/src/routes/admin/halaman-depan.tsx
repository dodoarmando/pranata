import { createFileRoute, getRouteApi, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type ChangeEvent, type FormEvent, useState } from 'react';
import {
  getHomeHero,
  getHomeWelcome,
  type HomeHero,
  listPositions,
  updateHomeHero,
  updateHomeWelcome,
  uploadHomeHeroPhoto,
  uploadHomeWelcomePhoto,
  type WelcomeFields,
} from '../../api/generated';
import { Notice, PageHead } from '../../components/admin';
import { Button } from '../../components/ui/button';
import { Card, CardContent, CardDescription, CardHeader } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { NativeSelect, NativeSelectOption } from '../../components/ui/native-select';
import { Textarea } from '../../components/ui/textarea';
import { tenureKindLabels } from '../../government';
import { villageToday } from '../../village';

// Server-only; null for what is not saved yet.
const fetchHome = createServerFn({ method: 'GET' }).handler(async () => {
  const [hero, welcome, positions] = await Promise.all([
    getHomeHero(),
    getHomeWelcome(),
    listPositions(),
  ]);
  return {
    hero: hero.status === 200 ? hero.data : null,
    welcome: welcome.status === 200 ? welcome.data : null,
    positions: positions.status === 200 ? positions.data : [],
  };
});

const photoOf = (data: FormData) => {
  const photo = data.get('photo');
  if (!(photo instanceof File)) throw new Error('Expected a photo file');
  return photo;
};

const uploadHeroPhotoFn = createServerFn({ method: 'POST' })
  .validator(photoOf)
  .handler(async ({ data }) => {
    const response = await uploadHomeHeroPhoto(data);
    return response.status === 201
      ? { status: 201, id: response.data.id }
      : { status: response.status };
  });

const uploadWelcomePhotoFn = createServerFn({ method: 'POST' })
  .validator(photoOf)
  .handler(async ({ data }) => {
    const response = await uploadHomeWelcomePhoto(data);
    return response.status === 201
      ? { status: 201, id: response.data.id }
      : { status: response.status };
  });

const saveHeroFn = createServerFn({ method: 'POST' })
  .validator((hero: HomeHero) => hero)
  .handler(async ({ data }) => (await updateHomeHero(data)).status);

const saveWelcomeFn = createServerFn({ method: 'POST' })
  .validator((welcome: WelcomeFields) => welcome)
  .handler(async ({ data }) => (await updateHomeWelcome(data)).status);

const root = getRouteApi('__root__');

export const Route = createFileRoute('/admin/halaman-depan')({
  loader: () => fetchHome(),
  head: () => ({ meta: [{ title: 'Halaman depan · Pranata' }] }),
  component: HomeSettings,
});

const MAX_PHOTO_BYTES = 10 << 20;
const QUICK_LINKS = 4;

const forbidden = 'Anda tidak memiliki izin menerbitkan konten.';

const photoErrors: Record<number, string> = {
  403: forbidden,
  413: 'Foto paling besar 10 MB.',
  415: 'Foto harus berupa berkas JPEG, PNG, atau WebP.',
  422: 'Gambar rusak atau lebih dari 40 megapiksel.',
};

const heroErrors: Record<number, string> = {
  403: forbidden,
  422: 'Periksa lagi isian: foto dan teks alternatifnya serta judul wajib diisi, label dan alamat tombol diisi keduanya atau dikosongkan keduanya, dan setiap alamat diawali "/", misalnya /profil.',
};

const welcomeErrors: Record<number, string> = {
  403: forbidden,
  422: 'Periksa lagi isian: foto dan teks alternatifnya serta sambutan (paling banyak 2.000 karakter) wajib diisi, dan penanda tangan harus pejabat yang sedang menjabat.',
};

const text = (form: FormData, name: string) => String(form.get(name) ?? '').trim();

function HomeSettings() {
  const { hero, welcome, positions } = Route.useLoaderData();
  const router = useRouter();
  const uploadHeroPhoto = useServerFn(uploadHeroPhotoFn);
  const uploadWelcomePhoto = useServerFn(uploadWelcomePhotoFn);
  const saveHero = useServerFn(saveHeroFn);
  const saveWelcome = useServerFn(saveWelcomeFn);
  const [heroPhoto, setHeroPhoto] = useState(hero?.photo ?? null);
  const [welcomePhoto, setWelcomePhoto] = useState(welcome?.photo ?? null);
  const [uploading, setUploading] = useState(false);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);

  // Tenures of active Positions serving today; the API checks the same.
  const today = villageToday(root.useLoaderData()?.timeZone);
  const signers = positions.flatMap((position) =>
    position.active
      ? position.tenures
          .filter(
            (tenure) => tenure.startDate <= today && (!tenure.endDate || tenure.endDate >= today),
          )
          .map((tenure) => ({ tenure, position }))
      : [],
  );

  const upload =
    (send: typeof uploadHeroPhoto, set: (id: string) => void) =>
    async (event: ChangeEvent<HTMLInputElement>) => {
      const file = event.currentTarget.files?.[0];
      event.currentTarget.value = '';
      if (!file) return;
      if (file.size > MAX_PHOTO_BYTES) {
        setMessage({ text: photoErrors[413] ?? '', error: true });
        return;
      }
      setUploading(true);
      const data = new FormData();
      data.set('photo', file);
      const result = await send({ data }).finally(() => setUploading(false));
      if (result.id) {
        set(result.id);
        setMessage({ text: 'Foto terunggah. Simpan untuk memakainya.', error: false });
      } else {
        setMessage({ text: photoErrors[result.status] ?? 'Gagal mengunggah foto.', error: true });
      }
    };

  async function onSaveHero(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!heroPhoto) {
      setMessage({ text: 'Unggah foto hero terlebih dahulu.', error: true });
      return;
    }
    const form = new FormData(event.currentTarget);
    const status = await saveHero({
      data: {
        photo: heroPhoto,
        photoAlt: text(form, 'heroAlt'),
        title: text(form, 'title'),
        subtitle: text(form, 'subtitle') || null,
        buttonLabel: text(form, 'buttonLabel') || null,
        buttonPath: text(form, 'buttonPath') || null,
        quickLinks: Array.from({ length: QUICK_LINKS }, (_, index) => ({
          label: text(form, `linkLabel${index}`),
          path: text(form, `linkPath${index}`),
        })).filter((link) => link.label || link.path),
      },
    });
    setMessage(
      status === 204
        ? { text: 'Hero dan tautan cepat tersimpan dan langsung tampil.', error: false }
        : { text: heroErrors[status] ?? 'Gagal menyimpan.', error: true },
    );
    if (status === 204) await router.invalidate();
  }

  async function onSaveWelcome(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!welcomePhoto) {
      setMessage({ text: 'Unggah foto kepala desa terlebih dahulu.', error: true });
      return;
    }
    const form = new FormData(event.currentTarget);
    const status = await saveWelcome({
      data: {
        photo: welcomePhoto,
        photoAlt: text(form, 'welcomeAlt'),
        message: text(form, 'message'),
        signer: text(form, 'signer'),
      },
    });
    setMessage(
      status === 204
        ? { text: 'Sambutan tersimpan dan langsung tampil.', error: false }
        : { text: welcomeErrors[status] ?? 'Gagal menyimpan.', error: true },
    );
    if (status === 204) await router.invalidate();
  }

  const preview = (id: string | null, className: string) =>
    id ? (
      <img src={`/media/${id}/480`} alt="" width={160} height={90} className={className} />
    ) : (
      <span className={`grid place-items-center text-muted-foreground ${className}`}>
        Belum ada
      </span>
    );

  return (
    <>
      <PageHead title="Halaman depan">
        Hero, tautan cepat, dan sambutan kepala desa di beranda situs publik. Perubahan langsung
        tampil begitu disimpan.
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <form onSubmit={onSaveHero} aria-label="Hero" className="grid gap-5">
        <Card>
          <CardHeader>
            <h2 className="text-base font-semibold">Hero</h2>
            <CardDescription>
              Foto lebar di bagian atas beranda, dengan judul dan kotak pencarian. JPEG, PNG, atau
              WebP, paling besar 10 MB; sebaiknya lebar 2.400 piksel.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid max-w-2xl gap-5 sm:grid-cols-2">
            <div className="flex flex-wrap items-center gap-4 sm:col-span-2">
              {preview(heroPhoto, 'aspect-video w-40 rounded-lg border object-cover')}
              <div className="grid gap-2">
                <Label htmlFor="hero-photo">Unggah foto</Label>
                <Input
                  id="hero-photo"
                  type="file"
                  accept="image/jpeg,image/png,image/webp"
                  onChange={upload(uploadHeroPhoto, setHeroPhoto)}
                  disabled={uploading}
                />
              </div>
            </div>
            <div className="grid gap-2 sm:col-span-2">
              <Label htmlFor="hero-alt">Teks alternatif foto</Label>
              <Input
                id="hero-alt"
                name="heroAlt"
                required
                maxLength={200}
                defaultValue={hero?.photoAlt}
              />
            </div>
            <div className="grid gap-2 sm:col-span-2">
              <Label htmlFor="hero-title">Judul</Label>
              <Input
                id="hero-title"
                name="title"
                required
                maxLength={100}
                defaultValue={hero?.title}
              />
            </div>
            <div className="grid gap-2 sm:col-span-2">
              <Label htmlFor="hero-subtitle">Subjudul</Label>
              <Input
                id="hero-subtitle"
                name="subtitle"
                maxLength={200}
                defaultValue={hero?.subtitle ?? ''}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="hero-button-label">Label tombol</Label>
              <Input
                id="hero-button-label"
                name="buttonLabel"
                maxLength={40}
                defaultValue={hero?.buttonLabel ?? ''}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="hero-button-path">Alamat tombol</Label>
              <Input
                id="hero-button-path"
                name="buttonPath"
                placeholder="/profil"
                maxLength={200}
                defaultValue={hero?.buttonPath ?? ''}
              />
            </div>
            <p className="text-muted-foreground sm:col-span-2">
              Tombol hanya tampil bila label dan alamatnya diisi.
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <h2 className="text-base font-semibold">Tautan cepat</h2>
            <CardDescription>
              Paling banyak empat pintasan di bawah kotak pencarian. Baris yang kosong diabaikan.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid max-w-2xl gap-5 sm:grid-cols-2">
            {Array.from({ length: QUICK_LINKS }, (_, index) => {
              const link = hero?.quickLinks?.[index];
              return (
                // A fixed set of rows, one per place.
                // biome-ignore lint/suspicious/noArrayIndexKey: see above
                <fieldset key={index} className="contents">
                  <legend className="sr-only">Tautan cepat {index + 1}</legend>
                  <div className="grid gap-2">
                    <Label htmlFor={`link-label-${index}`}>Label {index + 1}</Label>
                    <Input
                      id={`link-label-${index}`}
                      name={`linkLabel${index}`}
                      maxLength={40}
                      defaultValue={link?.label ?? ''}
                    />
                  </div>
                  <div className="grid gap-2">
                    <Label htmlFor={`link-path-${index}`}>Alamat {index + 1}</Label>
                    <Input
                      id={`link-path-${index}`}
                      name={`linkPath${index}`}
                      placeholder="/berita"
                      maxLength={200}
                      defaultValue={link?.path ?? ''}
                    />
                  </div>
                </fieldset>
              );
            })}
          </CardContent>
        </Card>
        <Button type="submit" size="lg" className="w-fit px-4" disabled={uploading}>
          Simpan hero
        </Button>
      </form>
      <form onSubmit={onSaveWelcome} aria-label="Sambutan" className="grid gap-5">
        <Card>
          <CardHeader>
            <h2 className="text-base font-semibold">Sambutan kepala desa</h2>
            <CardDescription>
              Foto potret, isi sambutan, dan pejabat yang menandatanganinya.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid max-w-2xl gap-5">
            {welcome && !welcome.signerServing && (
              <Notice error>
                Masa jabatan {welcome.signerName} sebagai {welcome.signerPosition} sudah berakhir.
                Pilih penanda tangan yang sedang menjabat.
              </Notice>
            )}
            <div className="flex flex-wrap items-center gap-4">
              {preview(welcomePhoto, 'aspect-[3/4] w-24 rounded-lg border object-cover')}
              <div className="grid gap-2">
                <Label htmlFor="welcome-photo">Unggah foto</Label>
                <Input
                  id="welcome-photo"
                  type="file"
                  accept="image/jpeg,image/png,image/webp"
                  onChange={upload(uploadWelcomePhoto, setWelcomePhoto)}
                  disabled={uploading}
                />
              </div>
            </div>
            <div className="grid gap-2">
              <Label htmlFor="welcome-alt">Teks alternatif foto</Label>
              <Input
                id="welcome-alt"
                name="welcomeAlt"
                required
                maxLength={200}
                defaultValue={welcome?.photoAlt}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="welcome-message">Sambutan</Label>
              <Textarea
                id="welcome-message"
                name="message"
                required
                rows={6}
                maxLength={2000}
                aria-describedby="welcome-message-hint"
                defaultValue={welcome?.message}
              />
              <p id="welcome-message-hint" className="text-muted-foreground">
                Teks biasa. Pisahkan paragraf dengan baris kosong.
              </p>
            </div>
            <div className="grid gap-2">
              <Label htmlFor="welcome-signer">Penanda tangan</Label>
              <NativeSelect
                id="welcome-signer"
                name="signer"
                required
                defaultValue={welcome?.signer ?? ''}
                className="w-full"
              >
                <NativeSelectOption value="" disabled>
                  Pilih pejabat yang sedang menjabat
                </NativeSelectOption>
                {welcome && !welcome.signerServing && (
                  <NativeSelectOption value={welcome.signer}>
                    {welcome.signerName} — {welcome.signerPosition} (masa jabatan berakhir)
                  </NativeSelectOption>
                )}
                {signers.map(({ tenure, position }) => (
                  <NativeSelectOption key={tenure.id} value={tenure.id}>
                    {tenure.name} — {position.name} ({tenureKindLabels[tenure.kind]})
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </div>
          </CardContent>
        </Card>
        <Button type="submit" size="lg" className="w-fit px-4" disabled={uploading}>
          Simpan sambutan
        </Button>
      </form>
    </>
  );
}
