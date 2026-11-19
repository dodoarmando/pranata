import { createFileRoute, getRouteApi, useRouter } from '@tanstack/react-router';
import { createServerFn, useServerFn } from '@tanstack/react-start';
import { type ChangeEvent, type FormEvent, useState } from 'react';
import {
  getVisionMission,
  updateVillageProfile,
  updateVisionMission,
  uploadVillageLogo,
  type VillageProfile,
  type VisionMission,
} from '../../api/generated';
import { Notice, PageHead } from '../../components/admin';
import { Button } from '../../components/ui/button';
import { Card, CardContent, CardDescription, CardHeader } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { Textarea } from '../../components/ui/textarea';
import { regencyShort, villageTitle } from '../../village';

const uploadLogoFn = createServerFn({ method: 'POST' })
  .validator((data: FormData) => {
    const logo = data.get('logo');
    if (!(logo instanceof File)) throw new Error('Expected a logo file');
    return logo;
  })
  .handler(async ({ data }) => {
    const response = await uploadVillageLogo(data);
    return response.status === 201
      ? { status: response.status, id: response.data.id }
      : { status: response.status };
  });

const saveProfileFn = createServerFn({ method: 'POST' })
  .validator((profile: VillageProfile) => profile)
  .handler(async ({ data }) => (await updateVillageProfile(data)).status);

const fetchVisionMission = createServerFn({ method: 'GET' }).handler(async () => {
  const response = await getVisionMission();
  return response.status === 200 ? response.data : null;
});

const saveVisionMissionFn = createServerFn({ method: 'POST' })
  .validator((visionMission: VisionMission) => visionMission)
  .handler(async ({ data }) => (await updateVisionMission(data)).status);

const MAX_LOGO_BYTES = 2 << 20;

const forbidden = 'Anda tidak memiliki izin mengelola data desa.';

const uploadErrors: Record<number, string> = {
  403: forbidden,
  413: 'Logo paling besar 2 MB.',
  415: 'Logo harus berupa berkas PNG atau SVG.',
  422: 'Gambar rusak atau lebih dari 40 megapiksel.',
};

const saveErrors: Record<number, string> = {
  403: forbidden,
  422: 'Periksa lagi isian: kode pos 5 angka, email dan telepon yang sah, serta lintang dan bujur yang diisi keduanya.',
};

const visionMissionErrors: Record<number, string> = {
  403: forbidden,
  422: 'Periksa lagi isian: visi dan 1–20 misi wajib diisi, masing-masing paling banyak 1.000 karakter, dan periode RPJMDes di antara 2000–2100 dengan tahun akhir sesudah tahun awal.',
};

const root = getRouteApi('__root__');

export const Route = createFileRoute('/admin/profil-desa')({
  loader: () => fetchVisionMission(),
  head: () => ({ meta: [{ title: 'Profil desa · Pranata' }] }),
  component: VillageProfilePage,
});

const text = (form: FormData, name: string) => String(form.get(name) ?? '').trim() || null;
const number = (form: FormData, name: string) => {
  const value = text(form, name);
  return value === null ? null : Number(value);
};

function VillageProfilePage() {
  const village = root.useLoaderData();
  const visionMission = Route.useLoaderData();
  const router = useRouter();
  const uploadLogo = useServerFn(uploadLogoFn);
  const saveProfile = useServerFn(saveProfileFn);
  const saveVisionMission = useServerFn(saveVisionMissionFn);
  const [logo, setLogo] = useState(village?.logo ?? null);
  const [uploading, setUploading] = useState(false);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);

  if (!village) {
    return (
      <>
        <PageHead title="Profil desa" />
        <Notice error>Kode Desa belum ditetapkan. Hubungi penyedia Pranata.</Notice>
      </>
    );
  }

  async function onLogo(event: ChangeEvent<HTMLInputElement>) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = '';
    if (!file) return;
    if (file.size > MAX_LOGO_BYTES) {
      setMessage({ text: uploadErrors[413] ?? '', error: true });
      return;
    }
    setUploading(true);
    const data = new FormData();
    data.set('logo', file);
    const result = await uploadLogo({ data }).finally(() => setUploading(false));
    if (result.id) {
      setLogo(result.id);
      setMessage({ text: 'Logo terunggah. Simpan untuk memakainya.', error: false });
    } else {
      setMessage({ text: uploadErrors[result.status] ?? 'Gagal mengunggah logo.', error: true });
    }
  }

  async function onSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const status = await saveProfile({
      data: {
        address: text(form, 'address'),
        postalCode: text(form, 'postalCode'),
        phone: text(form, 'phone'),
        email: text(form, 'email'),
        latitude: number(form, 'latitude'),
        longitude: number(form, 'longitude'),
        logo,
        pseNumber: text(form, 'pseNumber'),
      },
    });
    setMessage(
      status === 204
        ? { text: 'Profil desa tersimpan.', error: false }
        : { text: saveErrors[status] ?? 'Gagal menyimpan.', error: true },
    );
    if (status === 204) await router.invalidate();
  }

  async function onSaveVisionMission(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const status = await saveVisionMission({
      data: {
        vision: String(form.get('vision')),
        missions: String(form.get('missions')).split('\n'),
        startYear: Number(form.get('startYear')),
        endYear: Number(form.get('endYear')),
      },
    });
    setMessage(
      status === 204
        ? { text: 'Visi dan misi tersimpan.', error: false }
        : { text: visionMissionErrors[status] ?? 'Gagal menyimpan.', error: true },
    );
    if (status === 204) await router.invalidate();
  }

  const field = (name: keyof VillageProfile, label: string, props = {}) => (
    <div className="grid gap-2">
      <Label htmlFor={`profile-${name}`}>{label}</Label>
      <Input id={`profile-${name}`} name={name} defaultValue={village[name] ?? ''} {...props} />
    </div>
  );

  return (
    <>
      <PageHead title="Profil desa">
        Kontak kantor, logo, serta visi dan misi yang tampil di situs publik.
      </PageHead>
      {message && <Notice error={message.error}>{message.text}</Notice>}
      <Card role="region" aria-labelledby="identity-title">
        <CardHeader>
          <h2 id="identity-title" className="text-base font-semibold">
            {villageTitle(village)}
          </h2>
          <CardDescription>
            Kec. {village.district}, {regencyShort(village)}, {village.province}. Nama dan wilayah
            mengikuti Kode Desa {village.code} dan tidak dapat diubah di sini.
          </CardDescription>
        </CardHeader>
      </Card>
      <form onSubmit={onSave} aria-label="Profil desa" className="grid gap-5">
        <Card>
          <CardHeader>
            <h2 className="text-base font-semibold">Kontak kantor</h2>
          </CardHeader>
          <CardContent className="grid max-w-2xl gap-5 sm:grid-cols-2">
            <div className="sm:col-span-2">{field('address', 'Alamat kantor')}</div>
            {field('postalCode', 'Kode pos', { inputMode: 'numeric', maxLength: 5 })}
            {field('phone', 'Telepon', { type: 'tel', autoComplete: 'off' })}
            <div className="sm:col-span-2">
              {field('email', 'Email', { type: 'email', autoComplete: 'off' })}
            </div>
            {field('latitude', 'Lintang', { type: 'number', step: 'any', min: -90, max: 90 })}
            {field('longitude', 'Bujur', { type: 'number', step: 'any', min: -180, max: 180 })}
            <p className="text-muted-foreground sm:col-span-2">
              Lintang dan bujur kantor membuat tautan "Buka di Google Maps" di halaman Kontak. Salin
              dari Google Maps, misalnya -6.2000 dan 106.8166.
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <h2 className="text-base font-semibold">Logo</h2>
            <CardDescription>
              PNG atau SVG, paling besar 2 MB. Teks di dalam SVG harus sudah diubah menjadi kurva
              (outline), karena tidak ikut digambar.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap items-center gap-4">
            {logo ? (
              <img
                src={`/media/${logo}/480`}
                alt={`Logo ${villageTitle(village)}`}
                width={80}
                height={80}
                className="size-20 rounded-lg border object-contain p-1"
              />
            ) : (
              <span className="grid size-20 place-items-center rounded-lg border text-muted-foreground">
                Belum ada
              </span>
            )}
            <div className="grid gap-2">
              <Label htmlFor="profile-logo">Unggah logo</Label>
              <Input
                id="profile-logo"
                type="file"
                accept="image/png,image/svg+xml"
                onChange={onLogo}
                disabled={uploading}
              />
            </div>
            {logo && (
              <Button type="button" variant="outline" onClick={() => setLogo(null)}>
                Hapus logo
              </Button>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <h2 className="text-base font-semibold">Sistem</h2>
            <CardDescription>Tampil di kaki situs publik bila diisi.</CardDescription>
          </CardHeader>
          <CardContent className="grid max-w-md gap-5">
            {field('pseNumber', 'Nomor Tanda Daftar PSE')}
          </CardContent>
        </Card>
        <Button type="submit" size="lg" className="w-fit px-4" disabled={uploading}>
          Simpan
        </Button>
      </form>
      <form onSubmit={onSaveVisionMission} aria-label="Visi dan misi" className="grid gap-5">
        <Card>
          <CardHeader>
            <h2 className="text-base font-semibold">Visi dan misi</h2>
            <CardDescription>
              Tampil di halaman Profil situs publik. Versi sebelumnya tetap tercatat di Log audit.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid max-w-2xl gap-5 sm:grid-cols-2">
            <div className="grid gap-2 sm:col-span-2">
              <Label htmlFor="vision">Visi</Label>
              <Textarea
                id="vision"
                name="vision"
                required
                maxLength={1000}
                defaultValue={visionMission?.vision}
              />
            </div>
            <div className="grid gap-2 sm:col-span-2">
              <Label htmlFor="missions">Misi</Label>
              <Textarea
                id="missions"
                name="missions"
                required
                rows={5}
                aria-describedby="missions-hint"
                defaultValue={visionMission?.missions.join('\n')}
              />
              <p id="missions-hint" className="text-muted-foreground">
                Satu misi per baris, berurutan dari misi pertama.
              </p>
            </div>
            {(
              [
                ['startYear', 'RPJMDes dari tahun'],
                ['endYear', 'Sampai tahun'],
              ] as const
            ).map(([name, label]) => (
              <div key={name} className="grid gap-2">
                <Label htmlFor={name}>{label}</Label>
                <Input
                  id={name}
                  name={name}
                  type="number"
                  required
                  min={2000}
                  max={2100}
                  defaultValue={visionMission?.[name]}
                />
              </div>
            ))}
          </CardContent>
        </Card>
        <Button type="submit" size="lg" className="w-fit px-4">
          Simpan visi dan misi
        </Button>
      </form>
    </>
  );
}
