import { createFileRoute } from "@tanstack/react-router";
import { createIsomorphicFn } from "@tanstack/react-start";
import { setResponseStatus } from "@tanstack/react-start/server";
import { useEffect } from "react";
import { ApiError } from "@/api/fetcher";
import { getPublicVillage } from "@/api/generated/public/public";
import { SiteFooter } from "@/components/site/footer";
import { SiteHeader } from "@/components/site/header";
import { Hero } from "@/components/site/hero";
import { installYoutubeFacade } from "@/lib/youtube-facade";

// During SSR the response must carry 503 so crawlers and the HTML cache skip it.
const markUnavailable = createIsomorphicFn().server(() => {
	setResponseStatus(503);
});

export const Route = createFileRoute("/")({
	loader: async () => {
		try {
			return { village: await getPublicVillage() };
		} catch (error) {
			if (
				error instanceof ApiError &&
				error.code === "village_not_initialized"
			) {
				markUnavailable();
				return { village: null };
			}
			throw error;
		}
	},
	head: ({ loaderData }) => {
		const village = loaderData?.village;
		return {
			meta: [
				{
					title: village
						? `Desa ${village.name} — Situs Resmi Pemerintah Desa`
						: "Situs desa belum disiapkan",
				},
			],
		};
	},
	component: Home,
});

function Home() {
	const { village } = Route.useLoaderData();
	useEffect(() => installYoutubeFacade(), []);

	if (!village) {
		return (
			<main id="konten" className="mx-auto w-[min(100%-2rem,40rem)] py-24">
				<h1 className="text-2xl font-semibold">Situs desa belum disiapkan.</h1>
			</main>
		);
	}

	return (
		<>
			<SiteHeader
				name={village.name}
				regency={village.regency}
				province={village.province}
			/>
			<main id="konten">
				<Hero
					name={village.name}
					hero={village.hero}
					overviewHtml={village.overview_html}
				/>
			</main>
			<SiteFooter name={village.name} />
		</>
	);
}
