import { Node as TiptapNode } from "@tiptap/core";
import {
	Table,
	TableCell,
	TableHeader,
	TableRow,
} from "@tiptap/extension-table";
import {
	EditorContent,
	type NodeViewProps,
	NodeViewWrapper,
	ReactNodeViewRenderer,
	useEditor,
	useEditorState,
} from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import {
	BoldIcon,
	HeadingIcon,
	ImageIcon,
	ItalicIcon,
	LinkIcon,
	ListIcon,
	ListOrderedIcon,
	QuoteIcon,
	Redo2Icon,
	TableIcon,
	Trash2Icon,
	Undo2Icon,
	VideoIcon,
} from "lucide-react";
import { type FormEvent, useState } from "react";
import { useGetMedia } from "@/api/generated/media/media";
import type { MediaAsset, MediaVariant } from "@/api/generated/model";
import { ImageUpload } from "@/components/image-upload";
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
import { type RichTextDoc, sanitizeDoc } from "@/lib/rich-text";
import { cn } from "@/lib/utils";
import { parseYoutubeId } from "@/lib/youtube-id";

// Picks the variant closest to what the editor column shows.
export function pickVariant(
	variants: MediaVariant[],
): MediaVariant | undefined {
	return [...variants].sort(
		(a, b) => Math.abs(a.width - 720) - Math.abs(b.width - 720),
	)[0];
}

function AssetImageView({
	node,
	updateAttributes,
	deleteNode,
	selected,
}: NodeViewProps) {
	const assetId = String(node.attrs.assetId ?? "");
	const media = useGetMedia(assetId, { query: { enabled: assetId !== "" } });
	const variant = media.data ? pickVariant(media.data.variants) : undefined;
	const alt = typeof node.attrs.alt === "string" ? node.attrs.alt : "";

	return (
		<NodeViewWrapper
			as="figure"
			className={cn("rte-figure", selected && "rte-figure-selected")}
			data-drag-handle=""
		>
			{variant ? (
				<img src={variant.url} alt={alt} className="rte-image" />
			) : (
				<div className="rte-image-placeholder">
					{media.isError ? "Foto tidak dapat dimuat." : "Memuat foto…"}
				</div>
			)}
			<div className="flex items-center gap-2" contentEditable={false}>
				<Input
					value={alt}
					onChange={(event) => updateAttributes({ alt: event.target.value })}
					onKeyDown={(event) => {
						if (event.key === "Enter") event.preventDefault();
					}}
					placeholder="Deskripsi foto (untuk pembaca layar)"
					aria-label="Deskripsi foto"
				/>
				<Button
					type="button"
					variant="outline"
					size="icon"
					className="size-10 shrink-0 md:size-8"
					aria-label="Hapus foto"
					onClick={() => deleteNode()}
				>
					<Trash2Icon />
				</Button>
			</div>
		</NodeViewWrapper>
	);
}

// Image node that stores only the asset id; the URL is looked up for display.
const AssetImage = TiptapNode.create({
	name: "image",
	group: "block",
	atom: true,
	draggable: true,
	addAttributes() {
		return { assetId: { default: null }, alt: { default: null } };
	},
	parseHTML() {
		return [
			{
				tag: "img[data-asset-id]",
				getAttrs: (el) => ({
					assetId: el.getAttribute("data-asset-id"),
					alt: el.getAttribute("alt"),
				}),
			},
		];
	},
	renderHTML({ node }) {
		return [
			"img",
			{ "data-asset-id": node.attrs.assetId, alt: node.attrs.alt ?? "" },
		];
	},
	addNodeView() {
		return ReactNodeViewRenderer(AssetImageView);
	},
});

// YouTube node that stores only the 11-character video id. The editor shows a
// plain box, so writing never contacts YouTube.
const YoutubeVideo = TiptapNode.create({
	name: "youtube",
	group: "block",
	atom: true,
	draggable: true,
	addAttributes() {
		return { videoId: { default: null } };
	},
	parseHTML() {
		return [
			{
				tag: "div[data-youtube-id]",
				getAttrs: (el) => ({ videoId: el.getAttribute("data-youtube-id") }),
			},
		];
	},
	renderHTML({ node }) {
		return [
			"div",
			{ "data-youtube-id": node.attrs.videoId, class: "rte-youtube" },
			`Video YouTube (${node.attrs.videoId})`,
		];
	},
});

const extensions = [
	StarterKit.configure({
		// Only what the server allowlist keeps.
		heading: { levels: [2, 3, 4] },
		code: false,
		codeBlock: false,
		horizontalRule: false,
		strike: false,
		underline: false,
		link: {
			openOnClick: false,
			autolink: false,
			linkOnPaste: true,
			protocols: ["mailto", "tel"],
			HTMLAttributes: { rel: null, target: null, class: null },
		},
	}),
	Table.configure({ resizable: false }),
	TableRow,
	TableHeader,
	TableCell,
	AssetImage,
	YoutubeVideo,
];

function ToolButton({
	label,
	active,
	disabled,
	onClick,
	children,
}: Readonly<{
	label: string;
	active?: boolean;
	disabled?: boolean;
	onClick: () => void;
	children: React.ReactNode;
}>) {
	return (
		<Button
			type="button"
			variant={active ? "secondary" : "ghost"}
			size="icon"
			// 40 px targets on a phone, compact on desktop.
			className="size-10 shrink-0 md:size-8"
			aria-label={label}
			aria-pressed={active ?? false}
			title={label}
			disabled={disabled}
			onMouseDown={(event) => event.preventDefault()}
			onClick={onClick}
		>
			{children}
		</Button>
	);
}

function Divider() {
	return (
		<span aria-hidden="true" className="mx-1 h-5 w-px shrink-0 bg-border" />
	);
}

export function RichTextEditor({
	value,
	onChange,
	label = "Isi",
}: Readonly<{
	value: RichTextDoc | null;
	onChange: (doc: RichTextDoc) => void;
	label?: string;
}>) {
	const [linkOpen, setLinkOpen] = useState(false);
	const [videoOpen, setVideoOpen] = useState(false);
	const [imageOpen, setImageOpen] = useState(false);

	// `value` seeds the editor once; later edits flow out through onChange only.
	const [initial] = useState(() => value ?? undefined);
	const editor = useEditor({
		extensions,
		content: initial,
		immediatelyRender: false,
		editorProps: {
			attributes: {
				class: "rte-content",
				role: "textbox",
				"aria-multiline": "true",
				"aria-label": label,
			},
		},
		onUpdate: ({ editor: current }) => onChange(sanitizeDoc(current.getJSON())),
	});

	const state = useEditorState({
		editor,
		selector: ({ editor: e }) =>
			e
				? {
						h2: e.isActive("heading", { level: 2 }),
						h3: e.isActive("heading", { level: 3 }),
						h4: e.isActive("heading", { level: 4 }),
						bold: e.isActive("bold"),
						italic: e.isActive("italic"),
						bullet: e.isActive("bulletList"),
						ordered: e.isActive("orderedList"),
						quote: e.isActive("blockquote"),
						link: e.isActive("link"),
						table: e.isActive("table"),
						canUndo: e.can().undo(),
						canRedo: e.can().redo(),
					}
				: null,
	});

	if (!editor || !state) {
		return (
			<div className="h-72 rounded-lg border bg-muted/30" aria-hidden="true" />
		);
	}

	function insertImage(asset: MediaAsset) {
		editor
			?.chain()
			.focus()
			.insertContent({ type: "image", attrs: { assetId: asset.id, alt: null } })
			.run();
	}

	return (
		<div className="overflow-hidden rounded-lg border focus-within:ring-3 focus-within:ring-ring/50">
			<div
				role="toolbar"
				aria-label="Format teks"
				className="flex items-center gap-0.5 overflow-x-auto border-b bg-sidebar p-1"
			>
				<ToolButton
					label="Judul besar"
					active={state.h2}
					onClick={() =>
						editor.chain().focus().toggleHeading({ level: 2 }).run()
					}
				>
					<HeadingIcon />
				</ToolButton>
				<ToolButton
					label="Judul sedang"
					active={state.h3}
					onClick={() =>
						editor.chain().focus().toggleHeading({ level: 3 }).run()
					}
				>
					<span className="text-xs font-semibold">H3</span>
				</ToolButton>
				<ToolButton
					label="Judul kecil"
					active={state.h4}
					onClick={() =>
						editor.chain().focus().toggleHeading({ level: 4 }).run()
					}
				>
					<span className="text-xs font-semibold">H4</span>
				</ToolButton>
				<Divider />
				<ToolButton
					label="Tebal"
					active={state.bold}
					onClick={() => editor.chain().focus().toggleBold().run()}
				>
					<BoldIcon />
				</ToolButton>
				<ToolButton
					label="Miring"
					active={state.italic}
					onClick={() => editor.chain().focus().toggleItalic().run()}
				>
					<ItalicIcon />
				</ToolButton>
				<Divider />
				<ToolButton
					label="Daftar berpoin"
					active={state.bullet}
					onClick={() => editor.chain().focus().toggleBulletList().run()}
				>
					<ListIcon />
				</ToolButton>
				<ToolButton
					label="Daftar bernomor"
					active={state.ordered}
					onClick={() => editor.chain().focus().toggleOrderedList().run()}
				>
					<ListOrderedIcon />
				</ToolButton>
				<ToolButton
					label="Kutipan"
					active={state.quote}
					onClick={() => editor.chain().focus().toggleBlockquote().run()}
				>
					<QuoteIcon />
				</ToolButton>
				<Divider />
				<ToolButton
					label="Tautan"
					active={state.link}
					onClick={() => setLinkOpen(true)}
				>
					<LinkIcon />
				</ToolButton>
				<ToolButton
					label="Sisipkan tabel"
					active={state.table}
					onClick={() =>
						editor
							.chain()
							.focus()
							.insertTable({ rows: 3, cols: 3, withHeaderRow: true })
							.run()
					}
				>
					<TableIcon />
				</ToolButton>
				<ToolButton label="Sisipkan foto" onClick={() => setImageOpen(true)}>
					<ImageIcon />
				</ToolButton>
				<ToolButton
					label="Sisipkan video YouTube"
					onClick={() => setVideoOpen(true)}
				>
					<VideoIcon />
				</ToolButton>
				{state.table ? (
					<>
						<Divider />
						<Button
							type="button"
							variant="ghost"
							size="sm"
							className="h-10 shrink-0 md:h-7"
							onClick={() => editor.chain().focus().addRowAfter().run()}
						>
							+ Baris
						</Button>
						<Button
							type="button"
							variant="ghost"
							size="sm"
							className="h-10 shrink-0 md:h-7"
							onClick={() => editor.chain().focus().addColumnAfter().run()}
						>
							+ Kolom
						</Button>
						<Button
							type="button"
							variant="ghost"
							size="sm"
							className="h-10 shrink-0 md:h-7"
							onClick={() => editor.chain().focus().deleteTable().run()}
						>
							Hapus tabel
						</Button>
					</>
				) : null}
				<Divider />
				<ToolButton
					label="Urungkan"
					disabled={!state.canUndo}
					onClick={() => editor.chain().focus().undo().run()}
				>
					<Undo2Icon />
				</ToolButton>
				<ToolButton
					label="Ulangi"
					disabled={!state.canRedo}
					onClick={() => editor.chain().focus().redo().run()}
				>
					<Redo2Icon />
				</ToolButton>
			</div>
			<EditorContent editor={editor} />

			<LinkDialog
				open={linkOpen}
				onOpenChange={setLinkOpen}
				initial={String(editor.getAttributes("link").href ?? "")}
				onSubmit={(href) => {
					const chain = editor.chain().focus().extendMarkRange("link");
					if (href === "") chain.unsetLink().run();
					else chain.setLink({ href }).run();
				}}
			/>
			<VideoDialog
				open={videoOpen}
				onOpenChange={setVideoOpen}
				onSubmit={(videoId) =>
					editor
						.chain()
						.focus()
						.insertContent({ type: "youtube", attrs: { videoId } })
						.run()
				}
			/>
			<Dialog open={imageOpen} onOpenChange={setImageOpen}>
				<DialogContent>
					<DialogHeader>
						<DialogTitle>Sisipkan foto</DialogTitle>
						<DialogDescription>
							Ambil foto atau pilih dari galeri. Foto dikecilkan sebelum
							diunggah.
						</DialogDescription>
					</DialogHeader>
					<ImageUpload onUploaded={insertImage} />
					<DialogFooter showCloseButton />
				</DialogContent>
			</Dialog>
		</div>
	);
}

const SAFE_HREF = /^(?:https?:\/\/|mailto:|tel:)/i;

function LinkDialog({
	open,
	onOpenChange,
	initial,
	onSubmit,
}: Readonly<{
	open: boolean;
	onOpenChange: (open: boolean) => void;
	initial: string;
	onSubmit: (href: string) => void;
}>) {
	const [error, setError] = useState<string | null>(null);

	function submit(event: FormEvent<HTMLFormElement>) {
		event.preventDefault();
		// Portaled forms still bubble through the React tree.
		event.stopPropagation();
		const href = String(
			new FormData(event.currentTarget).get("href") ?? "",
		).trim();
		if (href !== "" && !SAFE_HREF.test(href)) {
			setError("Tautan harus diawali http://, https://, mailto: atau tel:.");
			return;
		}
		setError(null);
		onSubmit(href);
		onOpenChange(false);
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent>
				<form onSubmit={submit} className="grid gap-4">
					<DialogHeader>
						<DialogTitle>Tautan</DialogTitle>
						<DialogDescription>
							Pilih teks lebih dulu. Kosongkan isian untuk melepas tautan.
						</DialogDescription>
					</DialogHeader>
					<div className="grid gap-2">
						<Label htmlFor="link-href">Alamat tautan</Label>
						<Input
							id="link-href"
							name="href"
							defaultValue={initial}
							inputMode="url"
							placeholder="https://"
							className="h-10 md:h-8"
						/>
						{error ? (
							<p role="alert" className="text-sm text-destructive">
								{error}
							</p>
						) : null}
					</div>
					<DialogFooter>
						<Button type="submit">Terapkan</Button>
					</DialogFooter>
				</form>
			</DialogContent>
		</Dialog>
	);
}

function VideoDialog({
	open,
	onOpenChange,
	onSubmit,
}: Readonly<{
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onSubmit: (videoId: string) => void;
}>) {
	const [error, setError] = useState<string | null>(null);

	function submit(event: FormEvent<HTMLFormElement>) {
		event.preventDefault();
		// Portaled forms still bubble through the React tree.
		event.stopPropagation();
		const id = parseYoutubeId(
			String(new FormData(event.currentTarget).get("url") ?? ""),
		);
		if (!id) {
			setError("Tautan YouTube tidak dikenali.");
			return;
		}
		setError(null);
		onSubmit(id);
		onOpenChange(false);
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent>
				<form onSubmit={submit} className="grid gap-4">
					<DialogHeader>
						<DialogTitle>Video YouTube</DialogTitle>
						<DialogDescription>
							Tempel tautan video. Hanya kode videonya yang disimpan.
						</DialogDescription>
					</DialogHeader>
					<div className="grid gap-2">
						<Label htmlFor="video-url">Tautan video</Label>
						<Input
							id="video-url"
							name="url"
							inputMode="url"
							placeholder="https://www.youtube.com/watch?v=…"
							className="h-10 md:h-8"
						/>
						{error ? (
							<p role="alert" className="text-sm text-destructive">
								{error}
							</p>
						) : null}
					</div>
					<DialogFooter>
						<Button type="submit">Sisipkan</Button>
					</DialogFooter>
				</form>
			</DialogContent>
		</Dialog>
	);
}
