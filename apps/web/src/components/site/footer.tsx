export function SiteFooter({ name }: Readonly<{ name: string }>) {
	return (
		<footer className="bg-[#06281d] text-[#d4e5dc]">
			<div className="mx-auto w-[min(100%-2rem,75rem)] py-8 text-sm">
				<p>
					© {new Date().getFullYear()} Pemerintah Desa {name} · Dikelola dengan
					Pranata
				</p>
			</div>
		</footer>
	);
}
