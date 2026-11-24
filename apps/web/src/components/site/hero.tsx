import type { MediaRef } from "@/api/generated/model";

type HeroProps = {
	name: string;
	hero: MediaRef | null | undefined;
	overviewHtml: string | null | undefined;
};

function toSrcSet(hero: MediaRef): string {
	return [...hero.variants]
		.sort((a, b) => a.width - b.width)
		.map((v) => `${v.url} ${v.width}w`)
		.join(", ");
}

export function Hero({ name, hero, overviewHtml }: HeroProps) {
	const variants = hero
		? [...hero.variants].sort((a, b) => a.width - b.width)
		: [];
	const fallback = variants.at(-1);

	return (
		<section
			className="relative flex min-h-[28rem] items-end overflow-hidden bg-[#06281d] text-white"
			aria-labelledby="hero-judul"
		>
			{hero && fallback ? (
				<img
					src={fallback.url}
					srcSet={toSrcSet(hero)}
					sizes="100vw"
					alt={hero.alt ?? ""}
					fetchPriority="high"
					decoding="async"
					className="absolute inset-0 size-full object-cover"
				/>
			) : null}
			<div
				className="absolute inset-0 bg-gradient-to-b from-[#06281d]/20 to-[#06281d]/95"
				aria-hidden="true"
			/>
			<div className="relative mx-auto w-[min(100%-2rem,75rem)] py-10">
				<div className="grid max-w-3xl gap-4">
					<p className="text-xs font-semibold tracking-widest text-[#bfe3d2]">
						SELAMAT DATANG DI SITUS RESMI
					</p>
					<h1
						id="hero-judul"
						className="text-4xl leading-tight font-semibold tracking-tight"
					>
						Desa {name}
					</h1>
					{overviewHtml ? (
						<div
							className="overview text-[#e3efe9]"
							// biome-ignore lint/security/noDangerouslySetInnerHtml: overview_html is sanitized server-side from an allowlist
							dangerouslySetInnerHTML={{ __html: overviewHtml }}
						/>
					) : null}
				</div>
			</div>
		</section>
	);
}
