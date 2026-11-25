import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";

export interface Slide {
	tab: string;
	title: string;
	description: string;
	image?: { src: string; srcset?: string };
}

// R1-A ships built-in product slides. Photos from published Albums replace
// them in a later plan.
const SLIDES: readonly Slide[] = [
	{
		tab: "Profil",
		title: "Profil desa dalam satu tempat",
		description: "Perbarui gambaran umum dan foto desa kapan saja.",
	},
	{
		tab: "Ponsel",
		title: "Bisa dari ponsel",
		description:
			"Tulis dan unggah foto langsung dari lapangan, juga saat sinyal lemah.",
	},
	{
		tab: "Akses",
		title: "Setiap orang punya perannya",
		description:
			"Administrator mengatur siapa boleh menulis, menerbitkan, atau mengelola data.",
	},
];

// Moves `delta` slides from `current`, wrapping around both ends.
export function step(current: number, delta: number, count: number): number {
	return (((current + delta) % count) + count) % count;
}

const onDark =
	"text-white/70 hover:text-white data-active:text-white group-data-[variant=line]/tabs-list:data-active:after:bg-white";

// Slides change only on user action: nothing moves on its own (WCAG 2.2.2).
export function LoginCarousel({
	slides = SLIDES,
}: Readonly<{ slides?: readonly Slide[] }>) {
	const [current, setCurrent] = useState(0);
	const slide = slides[current];
	const go = (delta: number) =>
		setCurrent((c) => step(c, delta, slides.length));
	if (!slide) return null;

	return (
		<div className="relative flex flex-1 flex-col justify-between">
			<Tabs
				value={current}
				onValueChange={(value) => setCurrent(Number(value))}
			>
				<TabsList
					variant="line"
					aria-label="Tema foto"
					activateOnFocus
					className="h-auto w-full flex-wrap justify-start gap-6 p-0"
				>
					{slides.map((s, i) => (
						<TabsTrigger
							key={s.tab}
							value={i}
							className={cn("flex-none px-0", onDark)}
						>
							{s.tab}
						</TabsTrigger>
					))}
				</TabsList>
			</Tabs>
			<div>
				<div aria-live="polite" className="max-w-lg">
					<h2 className="mb-3 text-3xl font-medium">{slide.title}</h2>
					<p className="text-lg text-white/90">{slide.description}</p>
				</div>
				<div className="mt-6 flex items-center justify-between">
					<div aria-hidden="true" className="flex items-center gap-1.5">
						{slides.map((s, i) => (
							<span
								key={s.tab}
								className={cn(
									"h-1 rounded-full bg-white transition-[width,opacity] motion-reduce:transition-none",
									i === current ? "w-24 opacity-100" : "w-1 opacity-30",
								)}
							/>
						))}
					</div>
					<div className="flex gap-2">
						<Button
							type="button"
							variant="ghost"
							size="icon-lg"
							aria-label="Foto sebelumnya"
							className="rounded-full border-white/20 bg-white/10 text-white hover:bg-white/20 hover:text-white"
							onClick={() => go(-1)}
						>
							<ChevronLeftIcon />
						</Button>
						<Button
							type="button"
							variant="ghost"
							size="icon-lg"
							aria-label="Foto berikutnya"
							className="rounded-full border-white/20 bg-white/10 text-white hover:bg-white/20 hover:text-white"
							onClick={() => go(1)}
						>
							<ChevronRightIcon />
						</Button>
					</div>
				</div>
			</div>
		</div>
	);
}
