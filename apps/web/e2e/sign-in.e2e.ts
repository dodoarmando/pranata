import { gzipSync } from 'node:zlib';
import { expect, type Page, type Response, test } from '@playwright/test';

// `just e2e-run` creates an Administrator from the CLI and passes on the username and link.
const username = process.env.PRANATA_E2E_USERNAME ?? '';
const accessLink = process.env.PRANATA_E2E_ACCESS_LINK ?? '';
const password = 'kopi tubruk pagi hari';

test('the first Administrator sets a password from the Access Link, signs in and signs out', async ({
  page,
}) => {
  expect(username && accessLink, 'PRANATA_E2E_USERNAME and _ACCESS_LINK').toBeTruthy();
  await page.goto(accessLink);
  await page.getByLabel('Kata sandi baru').fill(password);
  await page.getByRole('button', { name: 'Simpan kata sandi' }).click();
  await expect(page.getByRole('heading', { name: 'Kata sandi tersimpan' })).toBeVisible();

  await page.getByRole('link', { name: 'masuk' }).click();
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();
  await expect(page.getByRole('navigation', { name: 'Menu admin' })).toContainText('Log audit');

  await page.getByText('Sistem').click();
  await page.getByRole('link', { name: 'Log audit' }).click();
  await expect(page.getByRole('heading', { name: 'Log audit' })).toBeVisible();
  await page.getByLabel('Pengguna').fill(username);
  await page.getByRole('button', { name: 'Saring' }).click();
  await expect(page).toHaveURL(new RegExp(`username=${username}`));
  await expect(page.getByRole('table')).toContainText('session.sign_in');
  await expect(page.getByRole('table')).toContainText('access_link.use');
  await expect(page.getByRole('table')).toContainText('{"remember":false}');

  await page.getByRole('button', { name: 'Keluar' }).click();
  await expect(page).toHaveURL(/\/masuk$/);
  await page.goto('/admin');
  await expect(page).toHaveURL(/\/masuk$/);
});

test('a wrong password is refused and the forgotten-password hint names the Administrator', async ({
  page,
}) => {
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill('bukan kata sandinya');
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('alert')).toHaveText('Nama pengguna atau kata sandi salah.');

  await page.getByText('Lupa kata sandi?').click();
  await expect(page.getByText(/Hubungi Administrator desa/)).toBeVisible();
});

test('the Administrator creates a User from an Access Link, composes a Role and deactivates the User', async ({
  page,
  browser,
}) => {
  const newcomer = `${username}k`;
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Pengguna aktif' })).toBeVisible();

  await page.goto('/admin/pengguna');
  await page.getByLabel('Nama pengguna').fill(newcomer);
  await page
    .getByRole('combobox', { name: 'Peran', exact: true })
    .selectOption({ label: 'Kontributor Web' });
  await page.getByRole('button', { name: 'Buat pengguna' }).click();
  const link = page.getByRole('textbox', { name: 'Tautan Akses' });
  await expect(link).toHaveValue(/\/akses#[0-9a-f]{64}$/);

  const newcomerPage = await (await browser.newContext()).newPage();
  await newcomerPage.goto(await link.inputValue());
  await newcomerPage.getByLabel('Kata sandi baru').fill(password);
  await newcomerPage.getByRole('button', { name: 'Simpan kata sandi' }).click();
  await expect(newcomerPage.getByRole('heading', { name: 'Kata sandi tersimpan' })).toBeVisible();

  const row = page.getByRole('row').filter({ hasText: newcomer });
  await expect(row.getByLabel(`Peran ${newcomer}`)).toHaveValue(/.+/);
  page.once('dialog', (dialog) => dialog.accept());
  await row.getByRole('button', { name: 'Nonaktifkan' }).click();
  await expect(row).toContainText('Nonaktif');
  await expect(row.getByRole('button', { name: 'Aktifkan' })).toBeVisible();

  await page.goto('/admin/peran');
  const newRole = page.getByRole('form', { name: 'Peran baru' });
  await newRole.getByLabel('Nama peran').fill(`Peran ${username}`);
  await newRole.getByLabel('Baca pengaduan').check();
  await newRole.getByRole('button', { name: 'Buat peran' }).click();
  const saved = page.getByRole('form', { name: `Peran ${username}` });
  await expect(saved.getByLabel('Baca pengaduan')).toBeChecked();
  await expect(saved.getByLabel('Baca log audit')).not.toBeChecked();
});

/** A platform authenticator that verifies the User without asking, as Chrome's DevTools offer. */
async function addVirtualAuthenticator(page: Page) {
  const devtools = await page.context().newCDPSession(page);
  await devtools.send('WebAuthn.enable');
  await devtools.send('WebAuthn.addVirtualAuthenticator', {
    options: {
      protocol: 'ctap2',
      transport: 'internal',
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });
}

test('a User adds a passkey on Akun saya and signs in with it instead of the password', async ({
  page,
}) => {
  await addVirtualAuthenticator(page);
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();

  await page.goto('/admin/akun');
  await page.getByLabel('Nama passkey').fill('Laptop uji');
  await page.getByRole('button', { name: 'Tambah passkey' }).click();
  await expect(page.getByRole('alert')).toHaveText('Passkey Laptop uji ditambahkan.');
  await expect(page.getByRole('cell', { name: 'Laptop uji', exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Keluar', exact: true }).click();
  await expect(page).toHaveURL(/\/masuk$/);
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByRole('button', { name: 'Masuk dengan passkey' }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();

  await page.goto(`/admin/log-audit?username=${username}`);
  await expect(page.getByRole('table')).toContainText('passkey.create');
  await expect(page.getByRole('table')).toContainText('"passkey":"Laptop uji"');
});

test('the Audit Log is exported as a ZIP with its one-time password shown once', async ({
  page,
}) => {
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();

  await page.goto(`/admin/log-audit?username=${username}`);
  await page.getByRole('button', { name: 'Ekspor CSV' }).click();
  await page.getByLabel('Alasan ekspor').fill('Uji ekspor');
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Ekspor', exact: true }).click();
  expect((await download).suggestedFilename()).toBe('log-audit.zip');
  await expect(page.getByRole('dialog', { name: 'Kata sandi berkas' })).toContainText(
    /\w{5}-\w{5}-\w{5}-\w{5}/,
  );
  await page.getByRole('button', { name: 'Selesai' }).click();

  await page.reload();
  await expect(page.getByRole('table')).toContainText('audit_log.export');
  await expect(page.getByRole('table')).toContainText('"reason":"Uji ekspor"');
});

test('the Vision and Mission saved on Profil desa shows on the public Profil page', async ({
  page,
}) => {
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();

  // A unique vision each run, since the local database outlives runs.
  const vision = `Desa yang mandiri ${Date.now()}`;
  await page.goto('/admin/profil-desa');
  const form = page.getByRole('form', { name: 'Visi dan misi' });
  await form.getByLabel('Visi').fill(vision);
  await form.getByLabel('Misi').fill('Melayani warga\n\nMembangun irigasi');
  await form.getByLabel('RPJMDes dari tahun').fill('2025');
  await form.getByLabel('Sampai tahun').fill('2031');
  await form.getByRole('button', { name: 'Simpan visi dan misi' }).click();
  await expect(page.getByText('Visi dan misi tersimpan.')).toBeVisible();

  await page.goto('/profil');
  const section = page.getByRole('region', { name: 'Visi dan misi' });
  await expect(section).toContainText('RPJMDes 2025–2031');
  await expect(section).toContainText(vision);
  await expect(section.getByRole('listitem')).toHaveText(['Melayani warga', 'Membangun irigasi']);
  await expect(page.getByRole('region', { name: /^Profil Desa / })).toContainText('11.01.01.2001');
});

test('a village manager builds a Hamlet with an RW and RT on Wilayah, then deactivates the RT', async ({
  page,
}) => {
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();

  // The local database outlives runs: a fresh Hamlet name, and an RW number unlikely to be taken.
  const hamlet = `Krajan ${Date.now()}`;
  const rw = String((Date.now() % 999) + 1).padStart(3, '0');
  await page.getByRole('link', { name: 'Wilayah' }).click();
  await expect(page.getByRole('heading', { name: 'Wilayah' })).toBeVisible();
  await page.getByRole('button', { name: 'Tambah dusun' }).click();
  await page.getByLabel('Nama dusun').fill(hamlet);
  await page.getByRole('button', { name: 'Simpan', exact: true }).click();
  const card = page.getByRole('region', { name: `Dusun ${hamlet}` });

  await card.getByRole('button', { name: 'Tambah RW' }).click();
  await page.getByLabel('Nomor RW').fill(rw);
  await page.getByRole('button', { name: 'Simpan', exact: true }).click();
  await expect(card.getByRole('table')).toContainText(`RW ${rw}`);

  await card.getByRole('button', { name: 'RT', exact: true }).click();
  await page.getByLabel('Nomor RT').fill('1');
  await page.getByLabel('Nama ketua (opsional)').fill('Pak Sastro');
  await page.getByRole('button', { name: 'Simpan', exact: true }).click();
  const rt = card.getByRole('row', { name: /RT 001/ });
  await expect(rt).toContainText('Pak Sastro');

  await rt.getByRole('button', { name: 'Ubah' }).click();
  await page.getByLabel('Aktif').uncheck();
  await page.getByRole('button', { name: 'Simpan', exact: true }).click();
  await expect(rt).toContainText('Nonaktif');
});

test('a village manager records a Hamlet head on Pemerintahan, shown publicly and linkable to a User', async ({
  page,
}) => {
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();

  // A fresh Hamlet each run brings a fresh Head-of-Hamlet Position to fill.
  const hamlet = `Kidul ${Date.now()}`;
  const head = `Ratna ${Date.now()}`;
  await page.getByRole('link', { name: 'Wilayah' }).click();
  await page.getByRole('button', { name: 'Tambah dusun' }).click();
  await page.getByLabel('Nama dusun').fill(hamlet);
  await page.getByRole('button', { name: 'Simpan', exact: true }).click();
  await expect(page.getByRole('region', { name: `Dusun ${hamlet}` })).toBeVisible();
  await page.getByRole('link', { name: 'Beranda' }).click();
  const missing = page.getByRole('region', { name: 'Data desa perlu dilengkapi' });
  await expect(missing).toContainText(`Kepala Dusun ${hamlet}`);

  await page.getByRole('link', { name: 'Pemerintahan' }).click();
  await expect(page.getByRole('heading', { name: 'Pemerintahan', exact: true })).toBeVisible();
  const card = page.getByRole('region', { name: `Kepala Dusun ${hamlet}` });
  await card.getByRole('button', { name: 'Tambah masa jabatan' }).click();
  await page.getByLabel('Nama', { exact: true }).fill(head);
  await page.getByLabel('NIPD (opsional)').fill('1101012001009');
  await page.getByLabel('Tanggal mulai').fill('2024-01-01');
  await page.getByLabel('Nomor SK').fill('141/07/2024');
  await page.getByLabel('Tanggal SK').fill('2023-12-20');
  await page.getByRole('button', { name: 'Simpan', exact: true }).click();
  await expect(card.getByRole('table')).toContainText(head);
  await expect(card.getByRole('table')).toContainText('1 Januari 2024 – sekarang');
  await page.getByRole('link', { name: 'Beranda' }).click();
  await expect(missing).not.toContainText(`Kepala Dusun ${hamlet}`);

  await page.goto('/pemerintahan');
  await expect(page.getByRole('region', { name: 'Perangkat desa' })).toContainText(
    `Kepala Dusun ${hamlet}`,
  );
  const hamlets = page.getByRole('region', { name: 'Dusun' });
  await expect(hamlets).toContainText(`Dusun ${hamlet}${head}`);
  await expect(page.getByRole('main')).not.toContainText('1101012001009');
  await expect(page.getByRole('main')).not.toContainText('141/07/2024');

  // The signed-in Administrator is that Official.
  await page.goto('/admin/pengguna');
  const official = page.getByLabel(`Pejabat ${username}`, { exact: true });
  await official.selectOption({ label: head });
  await page.reload();
  await expect(official.locator('option:checked')).toHaveText(head);
});

test('a village manager records an Institution with ordered members, listed publicly', async ({
  page,
}) => {
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();

  // A fresh name each run, since the local database outlives runs.
  const name = `Karang Taruna ${Date.now()}`;
  await page.getByRole('link', { name: 'Lembaga desa' }).click();
  await page.getByRole('button', { name: 'Tambah lembaga' }).click();
  await page.getByLabel('Nama lembaga').fill(name);
  await page.getByLabel('Deskripsi').fill('Wadah generasi muda desa.');
  for (const [index, [member, role]] of [
    ['Sinta Dewi', 'Sekretaris'],
    ['Fajar Nugraha', 'Ketua'],
  ].entries()) {
    await page.getByRole('button', { name: 'Tambah pengurus' }).click();
    await page.getByLabel(`Nama pengurus ${index + 1}`).fill(member ?? '');
    await page.getByLabel(`Jabatan pengurus ${index + 1}`).fill(role ?? '');
  }
  await page.getByRole('button', { name: 'Naikkan pengurus 2' }).click();
  await page.getByRole('button', { name: 'Simpan', exact: true }).click();
  const card = page.getByRole('region', { name });
  await expect(card).toContainText('Fajar Nugraha · KetuaSinta Dewi · Sekretaris');

  await page.goto('/pemerintahan');
  const institutions = page.getByRole('region', { name: 'Lembaga desa' });
  const item = institutions.getByRole('listitem').filter({ hasText: name });
  await item.getByText('2 pengurus/anggota').click();
  await expect(item).toContainText('Fajar NugrahaKetuaSinta DewiSekretaris');
});

test('a village manager records an Indeks Desa Edition, shown publicly as the latest', async ({
  page,
}) => {
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();

  // The last year allowed, so it is the latest. A year holds one Edition and the local
  // database outlives runs, so one a failed run left behind goes first.
  await page.getByRole('link', { name: 'Indeks Desa' }).click();
  const row = page.getByRole('row').filter({ hasText: '2100' });
  page.on('dialog', (dialog) => dialog.accept());
  if (await row.count()) {
    await row.getByRole('button', { name: 'Hapus' }).click();
    await expect(row).toHaveCount(0);
  }
  await page.getByRole('button', { name: 'Tambah edisi' }).click();
  await page.getByLabel('Tahun pengumpulan data').fill('2100');
  await page.getByLabel('Skor', { exact: true }).fill('72.48');
  await page.getByLabel('Status').selectOption({ label: 'Maju' });
  await page.getByLabel('Ekonomi').fill('63');
  await page.getByRole('button', { name: 'Simpan', exact: true }).click();
  await expect(row).toContainText('72,48Maju');

  await page.goto('/profil');
  const section = page.getByRole('region', { name: 'Indeks Desa' });
  await expect(section).toContainText('Desa Maju');
  await expect(section).toContainText('72,48 dari 100');
  await expect(section).toContainText('Ekonomi63,00');

  await page.goto('/admin/indeks-desa');
  await row.getByRole('button', { name: 'Hapus' }).click();
  await expect(page.getByText('Edisi dihapus.')).toBeVisible();
  await expect(row).toHaveCount(0);
});

test('a publisher writes and publishes a News Post, read on /berita until unpublished', async ({
  page,
}) => {
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();
  page.on('dialog', (dialog) => dialog.accept());
  // The local database outlives runs, so names are this run's own.
  const run = Date.now();
  const category = `Kegiatan ${run}`;
  const title = `Kerja bakti ${run}`;
  const cover = await page.screenshot({ clip: { x: 0, y: 0, width: 320, height: 180 } });

  await page.getByRole('link', { name: 'Berita' }).click();
  await page.getByLabel('Kategori baru').fill(category);
  await page.getByRole('button', { name: 'Tambah kategori' }).click();
  await expect(page.getByText('Kategori ditambahkan.')).toBeVisible();
  await page.getByRole('button', { name: 'Tulis berita' }).click();
  await page.getByLabel('Judul').fill(title);
  await page.getByRole('button', { name: 'Mulai menulis' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'Tulis berita' })).toBeVisible();
  const editor = page.url();

  await page.getByLabel('Kategori').selectOption({ label: category });
  await page.getByLabel('Ringkasan').fill('Warga membersihkan saluran air bersama-sama.');
  await page.getByLabel(/Unggah foto/).setInputFiles({
    name: 'sampul.png',
    mimeType: 'image/png',
    buffer: cover,
  });
  await expect(page.getByText('Belum ada foto')).toHaveCount(0);
  await page.getByLabel('Keterangan foto').fill('Warga membersihkan saluran');
  await expect(page.getByLabel('Teks alternatif')).toHaveValue('Warga membersihkan saluran');
  await page.getByRole('textbox', { name: 'Isi berita' }).click();
  await page.keyboard.type('Kerja bakti dimulai pukul tujuh pagi.');
  await expect(page.getByRole('status')).toContainText('Tersimpan otomatis');
  await page.getByRole('button', { name: 'Terbitkan' }).click();
  await expect(page.getByText('Berita terbit di situs.')).toBeVisible();

  await page.goto('/berita');
  await page.getByRole('link', { name: title }).click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(title);
  await expect(page.getByText('Kerja bakti dimulai pukul tujuh pagi.')).toBeVisible();
  await expect(page.getByRole('img', { name: 'Warga membersihkan saluran' })).toBeVisible();
  const post = page.url();

  await page.goto(editor);
  await page.getByRole('button', { name: 'Batalkan terbit' }).click();
  await expect(page.getByText('Berita kembali menjadi draf.')).toBeVisible();
  expect((await page.goto(post))?.status()).toBe(404);
  await expect(page.getByRole('heading', { name: 'Berita tidak ditemukan' })).toBeVisible();

  await page.goto(editor);
  await page.getByRole('button', { name: 'Hapus draf' }).click();
  await expect(page.getByRole('heading', { name: 'Berita', exact: true })).toBeVisible();
  await page
    .getByRole('listitem')
    .filter({ hasText: category })
    .getByRole('button', { name: 'Hapus' })
    .click();
  await expect(page.getByText('Kategori dihapus.')).toBeVisible();
});

test('a publisher publishes an Announcement, shown on the home page and on /pengumuman', async ({
  page,
}) => {
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();
  page.on('dialog', (dialog) => dialog.accept());
  // The local database outlives runs, so the title is this run's own.
  const title = `Pelayanan KTP libur ${Date.now()}`;

  await page.getByRole('link', { name: 'Pengumuman', exact: true }).click();
  await page.getByRole('button', { name: 'Tulis pengumuman' }).click();
  await page.getByLabel('Judul').fill(title);
  await page.getByRole('button', { name: 'Mulai menulis' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'Tulis pengumuman' })).toBeVisible();
  const editor = page.url();

  await page.getByLabel('Ringkasan').fill('Kantor desa tutup untuk pelayanan KTP.');
  await page.getByLabel('Mulai').fill('2020-01-01');
  await page.getByLabel('Berakhir').fill('2099-12-31');
  await page.getByRole('textbox', { name: 'Isi lengkap (opsional)' }).click();
  await page.keyboard.type('Pelayanan dibuka kembali hari Senin.');
  await expect(page.getByRole('status')).toContainText('Tersimpan otomatis');
  await page.getByRole('button', { name: 'Terbitkan' }).click();
  await expect(page.getByText('Pengumuman terbit di situs.')).toBeVisible();

  await page.goto('/');
  const card = page.getByRole('article').filter({ hasText: title });
  await expect(card.getByText('Berlaku', { exact: true })).toBeVisible();
  await page.goto('/pengumuman');
  await page
    .getByRole('article')
    .filter({ hasText: title })
    .getByRole('link', { name: /Selengkapnya/ })
    .click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(title);
  await expect(page.getByText('Pelayanan dibuka kembali hari Senin.')).toBeVisible();

  await page.goto(editor);
  await page.getByRole('button', { name: 'Batalkan terbit' }).click();
  await expect(page.getByText('Pengumuman kembali menjadi draf.')).toBeVisible();
  await page.getByRole('button', { name: 'Hapus draf' }).click();
  await expect(page.getByRole('heading', { name: 'Pengumuman', exact: true })).toBeVisible();
});

test('a publisher lists an Event on /agenda and calls it off, labelled Dibatalkan', async ({
  page,
}) => {
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();
  page.on('dialog', (dialog) => dialog.accept());
  // The local database outlives runs, so names are this run's own.
  const run = Date.now();
  const category = `Gotong royong ${run}`;
  const title = `Kerja bakti ${run}`;

  await page.getByRole('link', { name: 'Berita' }).click();
  await page.getByLabel('Kategori baru').fill(category);
  await page.getByRole('button', { name: 'Tambah kategori' }).click();
  await expect(page.getByText('Kategori ditambahkan.')).toBeVisible();
  await page.getByRole('link', { name: 'Agenda' }).click();
  await page.getByRole('button', { name: 'Tulis kegiatan' }).click();
  await page.getByLabel('Judul').fill(title);
  await page.getByRole('button', { name: 'Mulai menulis' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'Tulis kegiatan' })).toBeVisible();
  const editor = page.url();

  await page.getByLabel('Kategori').selectOption({ label: category });
  await page.getByLabel('Mulai').fill('2099-01-02T07:00');
  await page.getByLabel('Tempat').fill('Balai desa');
  await expect(page.getByRole('status')).toContainText('Tersimpan otomatis');
  await page.getByRole('button', { name: 'Terbitkan' }).click();
  await expect(page.getByText('Kegiatan terbit di Agenda.')).toBeVisible();

  await page.goto('/agenda');
  const upcoming = page.getByRole('region', { name: 'Akan datang' });
  const card = upcoming.getByRole('article').filter({ hasText: title });
  await expect(card.getByText('Jumat, 2 Januari 2099, 07.00', { exact: false })).toBeVisible();
  await expect(card.getByText('Dibatalkan')).toHaveCount(0);

  await page.goto(editor);
  await page.getByLabel('Dibatalkan').check();
  await page.getByRole('button', { name: 'Simpan' }).click();
  await expect(page.getByRole('status')).toHaveText('Terbit di Agenda.');
  await page.goto('/agenda');
  await expect(card.getByText('Dibatalkan')).toBeVisible();

  await page.goto(editor);
  await page.getByRole('button', { name: 'Batalkan terbit' }).click();
  await expect(page.getByText('Kegiatan kembali menjadi draf.')).toBeVisible();
  await page.getByRole('button', { name: 'Hapus draf' }).click();
  await expect(page.getByRole('heading', { name: 'Agenda', exact: true })).toBeVisible();
  await page.getByRole('link', { name: 'Berita' }).click();
  await page
    .getByRole('listitem')
    .filter({ hasText: category })
    .getByRole('button', { name: 'Hapus' })
    .click();
  await expect(page.getByText('Kategori dihapus.')).toBeVisible();
});

test('every page has the stylesheet, and the public site stays light', async ({ page }) => {
  // Admin code is split into route chunks the public home page never fetches.
  const scripts: Promise<string>[] = [];
  const collect = (response: Response) => {
    if (response.ok() && response.request().resourceType() === 'script') {
      scripts.push(response.text());
    }
  };
  page.on('response', collect);
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  page.off('response', collect);
  for (const script of await Promise.all(scripts)) expect(script).not.toContain('Menu admin');

  const href = await page.locator('link[rel="stylesheet"]').first().getAttribute('href');
  const css = await (await page.request.get(href ?? '')).body();
  expect(gzipSync(css).length, 'gzipped CSS bytes').toBeLessThanOrEqual(30 * 1024);

  // Geist reaches the page only through the stylesheet.
  const expectStyled = async (path: string) => {
    await page.goto(path);
    const font = await page.evaluate(() => getComputedStyle(document.body).fontFamily);
    expect(font, path).toContain('Geist');
  };
  await expectStyled('/');
  await expectStyled('/akses');
  await expectStyled('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();
  for (const path of [
    '/admin',
    '/admin/akun',
    '/admin/pemerintahan',
    '/admin/lembaga-desa',
    '/admin/berita',
    '/admin/pengguna',
    '/admin/peran',
    '/admin/log-audit',
  ]) {
    await expectStyled(path);
  }
});
