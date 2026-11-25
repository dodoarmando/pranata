import { Link } from "@tanstack/react-router";
import { ArrowLeftIcon } from "lucide-react";
import type { ReactNode } from "react";
import { LoginCarousel } from "@/components/login-carousel";

// Static scene for the sign-in side panel. Real village photos can replace it
// once the village has uploaded some.
function Scene() {
	return (
		<svg
			aria-hidden="true"
			viewBox="0 0 800 600"
			preserveAspectRatio="xMidYMid slice"
			className="absolute inset-0 size-full"
		>
			<defs>
				<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
					<stop offset="0" stopColor="#bfdce4" />
					<stop offset="1" stopColor="#e8e3c6" />
				</linearGradient>
			</defs>
			<rect width="800" height="600" fill="url(#sky)" />
			<circle cx="620" cy="150" r="52" fill="#fff3c4" opacity="0.9" />
			<path
				d="M0 360 Q160 280 320 340 T640 320 T800 350 V600 H0Z"
				fill="#8fb996"
			/>
			<path d="M0 420 Q200 350 400 410 T800 400 V600 H0Z" fill="#5f9d7a" />
			<path d="M0 490 Q220 440 420 480 T800 470 V600 H0Z" fill="#006d4b" />
		</svg>
	);
}

export function AuthShell({
	title,
	subtitle,
	children,
}: Readonly<{ title: string; subtitle: string; children: ReactNode }>) {
	return (
		<div className="flex min-h-dvh">
			<section aria-label="Galeri" className="hidden flex-1 p-4 lg:flex">
				<div className="relative flex flex-1 flex-col overflow-hidden rounded-xl p-6 text-white">
					<Scene />
					<div className="pointer-events-none absolute inset-0 bg-linear-to-b from-[#06281d80] via-transparent to-[#06281db3]" />
					<LoginCarousel />
				</div>
			</section>
			<main
				id="konten"
				className="flex flex-1 flex-col items-center justify-between gap-10 px-5 py-6"
			>
				<div className="w-full max-w-sm flex-1 content-center">
					<header className="mb-6 text-center">
						<span
							aria-hidden="true"
							className="mx-auto mb-4 grid size-12 place-items-center rounded-xl bg-primary text-lg font-semibold text-primary-foreground"
						>
							P
						</span>
						<h1 className="text-2xl font-semibold">{title}</h1>
						<p className="mt-1 text-sm text-muted-foreground">{subtitle}</p>
					</header>
					{children}
				</div>
				<footer className="flex flex-col items-center gap-2 text-sm">
					<Link
						to="/"
						className="inline-flex items-center gap-1.5 font-medium text-primary hover:underline"
					>
						<ArrowLeftIcon className="size-4" />
						Kembali ke situs desa
					</Link>
					<small className="text-muted-foreground">
						Dikelola dengan Pranata
					</small>
				</footer>
			</main>
		</div>
	);
}
