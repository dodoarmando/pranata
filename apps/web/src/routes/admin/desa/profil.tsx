import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";
import type { MediaRef, Profile } from "@/api/generated/model";
import {
	getGetVillageProfileQueryKey,
	useGetVillageProfile,
	usePutVillageProfile,
} from "@/api/generated/village/village";
import { ImageUpload } from "@/components/image-upload";
import { pickVariant, RichTextEditor } from "@/components/rich-text-editor";
import { Button } from "@/components/ui/button";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { messageFor } from "@/lib/errors";
import { asDoc, isEmptyDoc, type RichTextDoc } from "@/lib/rich-text";

export const Route = createFileRoute("/admin/desa/profil")({
	head: () => ({ meta: [{ title: "Profil desa · Admin Pranata" }] }),
	component: ProfilePage,
});

function ProfilePage() {
	const profile = useGetVillageProfile();

	if (profile.isError) {
		return (
			<p role="alert" className="text-destructive">
				{messageFor(profile.error.code)}
			</p>
		);
	}
	if (!profile.data) {
		return (
			<div className="mx-auto flex max-w-3xl flex-col gap-4">
				<Skeleton className="h-8 w-48" />
				<Skeleton className="h-80 w-full" />
			</div>
		);
	}
	return <ProfileForm profile={profile.data} />;
}

function ProfileForm({ profile }: Readonly<{ profile: Profile }>) {
	const queryClient = useQueryClient();
	const [overview, setOverview] = useState<RichTextDoc | null>(() =>
		asDoc(profile.overview),
	);
	const [hero, setHero] = useState<MediaRef | null>(profile.hero ?? null);
	// Opaque concurrency token: sent back exactly as received, never parsed.
	const [updatedAt, setUpdatedAt] = useState(profile.updated_at);
	const save = usePutVillageProfile({
		mutation: {
			onSuccess: (saved) => {
				setUpdatedAt(saved.updated_at);
				queryClient.setQueryData(getGetVillageProfileQueryKey(), saved);
				toast.success("Profil desa disimpan.");
			},
			onError: (error) => toast.error(messageFor(error.code)),
		},
	});

	const heroVariant = hero ? pickVariant(hero.variants) : undefined;

	return (
		<form
			className="mx-auto flex max-w-3xl flex-col gap-4"
			onSubmit={(event) => {
				event.preventDefault();
				save.mutate({
					data: {
						overview:
							overview && !isEmptyDoc(overview) ? { ...overview } : null,
						hero_asset_id: hero?.id ?? null,
						updated_at: updatedAt,
					},
				});
			}}
		>
			<div>
				<h2 className="text-xl font-semibold">Profil desa</h2>
				<p className="text-sm text-muted-foreground">
					Perubahan langsung tampil di beranda situs saat disimpan.
				</p>
			</div>

			<Card>
				<CardHeader>
					<CardTitle>Gambaran umum</CardTitle>
					<CardDescription>
						Pengantar singkat tentang desa di beranda situs.
					</CardDescription>
				</CardHeader>
				<CardContent>
					<RichTextEditor
						value={overview}
						onChange={setOverview}
						label="Gambaran umum"
					/>
				</CardContent>
			</Card>

			<Card>
				<CardHeader>
					<CardTitle>Foto hero</CardTitle>
					<CardDescription>
						Foto besar di bagian atas beranda. JPEG, PNG, atau WebP.
					</CardDescription>
				</CardHeader>
				<CardContent className="flex flex-col gap-4">
					{heroVariant ? (
						<img
							src={heroVariant.url}
							alt="Foto hero saat ini"
							className="max-h-64 w-full rounded-lg object-cover"
						/>
					) : (
						<p className="text-sm text-muted-foreground">
							Belum ada foto hero.
						</p>
					)}
					<ImageUpload
						multiple={false}
						label={hero ? "Ganti foto hero" : "Unggah foto hero"}
						onUploaded={(asset) =>
							setHero({ id: asset.id, variants: asset.variants })
						}
					/>
					{hero ? (
						<div>
							<Button
								type="button"
								variant="ghost"
								size="sm"
								onClick={() => setHero(null)}
							>
								Hapus foto hero
							</Button>
						</div>
					) : null}
				</CardContent>
			</Card>

			<div className="sticky bottom-0 -mx-4 border-t bg-background/95 p-3 backdrop-blur md:static md:mx-0 md:border-0 md:bg-transparent md:p-0">
				<Button type="submit" size="lg" disabled={save.isPending}>
					{save.isPending ? "Menyimpan…" : "Simpan"}
				</Button>
			</div>
		</form>
	);
}
