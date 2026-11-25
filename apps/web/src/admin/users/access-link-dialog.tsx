import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export interface IssuedLink {
	displayName: string;
	url: string;
}

// Shows an Access Link once. It is never fetched again, so the dialog is the
// only chance to hand it over. No wa.me link: that URL would send the token
// to Meta's web servers before WhatsApp's end-to-end encryption applies, so
// sharing goes through the device's share sheet instead.
export function AccessLinkDialog({
	link,
	onClose,
}: Readonly<{ link: IssuedLink | null; onClose: () => void }>) {
	const canShare = typeof navigator !== "undefined" && "share" in navigator;

	async function copy(url: string) {
		try {
			await navigator.clipboard.writeText(url);
			toast.success("Tautan disalin.");
		} catch {
			toast.error("Tautan tidak dapat disalin. Salin secara manual.");
		}
	}

	async function share(url: string) {
		try {
			await navigator.share({ text: url });
		} catch {
			// Closing the share sheet rejects the promise; nothing to report.
		}
	}

	return (
		<Dialog
			open={link !== null}
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
		>
			<DialogContent>
				{link ? (
					<>
						<DialogHeader>
							<DialogTitle>Tautan akses untuk {link.displayName}</DialogTitle>
							<DialogDescription>
								Berlaku 24 jam dan hanya bisa dipakai sekali. Tautan ini tidak
								ditampilkan lagi.
							</DialogDescription>
						</DialogHeader>
						<div className="grid gap-2">
							<Label htmlFor="access-link-url" className="sr-only">
								Tautan akses
							</Label>
							<Input
								id="access-link-url"
								readOnly
								value={link.url}
								onFocus={(event) => event.currentTarget.select()}
							/>
						</div>
						<DialogFooter>
							{canShare ? (
								<Button
									type="button"
									variant="outline"
									onClick={() => void share(link.url)}
								>
									Bagikan
								</Button>
							) : null}
							<Button
								type="button"
								variant="outline"
								onClick={() => void copy(link.url)}
							>
								Salin tautan
							</Button>
							<Button type="button" onClick={onClose}>
								Selesai
							</Button>
						</DialogFooter>
					</>
				) : null}
			</DialogContent>
		</Dialog>
	);
}
