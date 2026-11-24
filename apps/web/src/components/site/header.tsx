type SiteHeaderProps = {
	name: string;
	regency: string;
	province: string;
};

export function SiteHeader({ name, regency, province }: SiteHeaderProps) {
	return (
		<>
			<a
				href="#konten"
				className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-4 focus:z-50 focus:rounded-sm focus:bg-primary focus:px-4 focus:py-2 focus:text-primary-foreground"
			>
				Lewati ke konten
			</a>
			<header className="border-b bg-background">
				<div className="mx-auto flex min-h-16 w-[min(100%-2rem,75rem)] items-center py-2">
					<p className="leading-tight">
						<strong className="block text-lg">Desa {name}</strong>
						<small className="block text-xs text-muted-foreground">
							{regency}, {province}
						</small>
					</p>
				</div>
			</header>
		</>
	);
}
