import { CheckCircle2Icon, ImagePlusIcon, RotateCcwIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { ApiError } from "@/api/fetcher";
import { getMedia, useUploadMedia } from "@/api/generated/media/media";
import type { MediaAsset } from "@/api/generated/model";
import { Button } from "@/components/ui/button";
import { FilePicker } from "@/components/ui/file-picker";
import { Progress } from "@/components/ui/progress";
import { downscaleImage } from "@/lib/downscale-image";
import { messageFor } from "@/lib/errors";
import { POLL_TIMEOUT_MS, pollDelay } from "@/lib/poll";

type Phase =
	| "queued"
	| "resizing"
	| "uploading"
	| "processing"
	| "ready"
	| "failed";

interface Item {
	key: number;
	file: File;
	phase: Phase;
	error: string | null;
}

const PHASE_LABEL: Record<Phase, string> = {
	queued: "Menunggu",
	resizing: "Mengecilkan foto…",
	uploading: "Mengunggah…",
	processing: "Memproses…",
	ready: "Selesai",
	failed: "Gagal",
};

const PHASE_VALUE: Record<Phase, number> = {
	queued: 0,
	resizing: 15,
	uploading: 45,
	processing: 75,
	ready: 100,
	failed: 0,
};

const OFFLINE = "Koneksi terputus. Periksa sinyal lalu coba lagi.";
const TIMEOUT =
	"Foto belum selesai diproses setelah 2 menit. Coba lagi beberapa saat lagi.";
const FAILED = "Foto gagal diproses. Coba foto lain.";

function errorText(error: unknown): string {
	return error instanceof ApiError ? messageFor(error.code) : OFFLINE;
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

export function ImageUpload({
	onUploaded,
	multiple = true,
	label = "Pilih foto",
}: Readonly<{
	onUploaded: (asset: MediaAsset) => void;
	multiple?: boolean;
	label?: string;
}>) {
	const [items, setItems] = useState<Item[]>([]);
	const upload = useUploadMedia();
	const queue = useRef<Item[]>([]);
	const running = useRef(false);
	const nextKey = useRef(0);
	const alive = useRef(true);
	const onUploadedRef = useRef(onUploaded);
	onUploadedRef.current = onUploaded;

	useEffect(() => {
		alive.current = true;
		return () => {
			alive.current = false;
		};
	}, []);

	const patch = useCallback((key: number, change: Partial<Item>) => {
		if (!alive.current) return;
		setItems((all) =>
			all.map((item) => (item.key === key ? { ...item, ...change } : item)),
		);
	}, []);

	// Polls until the server finished the image jobs; tolerates dropped
	// connections because the signal on a phone comes and goes.
	const waitUntilReady = useCallback(
		async (first: MediaAsset): Promise<MediaAsset> => {
			let asset = first;
			const started = Date.now();
			for (let attempt = 0; asset.status === "pending"; attempt++) {
				if (!alive.current) return asset;
				if (Date.now() - started > POLL_TIMEOUT_MS) throw new Error(TIMEOUT);
				await sleep(pollDelay(attempt));
				try {
					asset = await getMedia(first.id);
				} catch (error) {
					if (error instanceof ApiError) throw error;
				}
			}
			return asset;
		},
		[],
	);

	const process = useCallback(
		async (item: Item) => {
			try {
				patch(item.key, { phase: "resizing", error: null });
				const blob = await downscaleImage(item.file);
				patch(item.key, { phase: "uploading" });
				const created = await upload.mutateAsync({
					data: { file: blob, kind: "image" },
				});
				patch(item.key, { phase: "processing" });
				const asset = await waitUntilReady(created);
				if (asset.status === "failed") {
					patch(item.key, { phase: "failed", error: FAILED });
					return;
				}
				if (asset.status === "ready") {
					patch(item.key, { phase: "ready" });
					onUploadedRef.current(asset);
				}
			} catch (error) {
				const message =
					error instanceof Error && error.message === TIMEOUT
						? TIMEOUT
						: errorText(error);
				patch(item.key, { phase: "failed", error: message });
			}
		},
		[patch, upload.mutateAsync, waitUntilReady],
	);

	// Files go up one at a time so a slow connection is not split across many
	// requests that would all stall.
	const drain = useCallback(async () => {
		if (running.current) return;
		running.current = true;
		try {
			for (
				let next = queue.current.shift();
				next;
				next = queue.current.shift()
			) {
				await process(next);
			}
		} finally {
			running.current = false;
		}
	}, [process]);

	function add(files: File[]) {
		const added = files.map<Item>((file) => ({
			key: nextKey.current++,
			file,
			phase: "queued",
			error: null,
		}));
		setItems((all) => [...all, ...added]);
		queue.current.push(...added);
		void drain();
	}

	function retry(item: Item) {
		patch(item.key, { phase: "queued", error: null });
		queue.current.push(item);
		void drain();
	}

	return (
		<div className="flex flex-col gap-3">
			<FilePicker
				accept="image/*"
				multiple={multiple}
				onFiles={add}
				variant="outline"
				size="lg"
				className="h-11 w-full sm:w-auto"
			>
				<ImagePlusIcon />
				{label}
			</FilePicker>
			{items.length > 0 ? (
				<ul className="flex flex-col gap-3" aria-live="polite">
					{items.map((item) => (
						<li key={item.key} className="flex flex-col gap-1.5 text-sm">
							<div className="flex items-center justify-between gap-2">
								<span className="min-w-0 truncate">{item.file.name}</span>
								<span className="flex shrink-0 items-center gap-1 text-muted-foreground">
									{item.phase === "ready" ? (
										<CheckCircle2Icon className="size-4 text-primary" />
									) : null}
									{PHASE_LABEL[item.phase]}
								</span>
							</div>
							<Progress
								value={PHASE_VALUE[item.phase]}
								aria-label={`Kemajuan unggah ${item.file.name}`}
							/>
							{item.error ? (
								<div className="flex items-center justify-between gap-2">
									<p role="alert" className="text-destructive">
										{item.error}
									</p>
									<Button
										type="button"
										variant="outline"
										size="sm"
										onClick={() => retry(item)}
									>
										<RotateCcwIcon />
										Coba lagi
									</Button>
								</div>
							) : null}
						</li>
					))}
				</ul>
			) : null}
		</div>
	);
}
