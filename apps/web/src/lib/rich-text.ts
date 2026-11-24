// Minimal hand-written shape of the Tiptap/ProseMirror document the API stores
// (the generated client types it as a generic object).
export type RichTextMark =
	| { type: "bold" }
	| { type: "italic" }
	| { type: "link"; attrs: { href: string } };

export interface RichTextNode {
	type: string;
	attrs?: Record<string, unknown>;
	content?: RichTextNode[];
	marks?: RichTextMark[];
	text?: string;
}

export interface RichTextDoc {
	type: "doc";
	content: RichTextNode[];
}

const CONTAINERS = new Set([
	"paragraph",
	"bulletList",
	"orderedList",
	"listItem",
	"blockquote",
	"table",
	"tableRow",
	"tableHeader",
	"tableCell",
	"heading",
]);

const VIDEO_ID = /^[A-Za-z0-9_-]{11}$/;
const ASSET_ID =
	/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const HREF = /^(?:https?:\/\/|mailto:|tel:)/i;

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function cleanMarks(raw: unknown): RichTextMark[] {
	if (!Array.isArray(raw)) return [];
	const types = raw.filter(isRecord);
	const out: RichTextMark[] = [];
	const link = types.find(
		(m) =>
			m.type === "link" &&
			isRecord(m.attrs) &&
			typeof m.attrs.href === "string" &&
			HREF.test(m.attrs.href.trim()),
	);
	if (link && isRecord(link.attrs) && typeof link.attrs.href === "string") {
		out.push({ type: "link", attrs: { href: link.attrs.href.trim() } });
	}
	if (types.some((m) => m.type === "bold")) out.push({ type: "bold" });
	if (types.some((m) => m.type === "italic")) out.push({ type: "italic" });
	return out;
}

function cleanChildren(node: unknown): RichTextNode[] {
	if (!isRecord(node) || !Array.isArray(node.content)) return [];
	return node.content.flatMap(cleanNode);
}

function cleanNode(node: unknown): RichTextNode[] {
	if (!isRecord(node)) return [];
	const type = node.type;
	if (typeof type !== "string") return cleanChildren(node);
	const attrs = isRecord(node.attrs) ? node.attrs : {};

	if (type === "text") {
		if (typeof node.text !== "string" || node.text === "") return [];
		const marks = cleanMarks(node.marks);
		return [
			marks.length > 0
				? { type: "text", text: node.text, marks }
				: { type: "text", text: node.text },
		];
	}
	if (type === "hardBreak") return [{ type: "hardBreak" }];
	if (type === "image") {
		if (typeof attrs.assetId !== "string" || !ASSET_ID.test(attrs.assetId)) {
			return [];
		}
		const clean: Record<string, unknown> = { assetId: attrs.assetId };
		if (typeof attrs.alt === "string" && attrs.alt !== "") {
			clean.alt = attrs.alt;
		}
		return [{ type: "image", attrs: clean }];
	}
	if (type === "youtube") {
		return typeof attrs.videoId === "string" && VIDEO_ID.test(attrs.videoId)
			? [{ type: "youtube", attrs: { videoId: attrs.videoId } }]
			: [];
	}
	if (CONTAINERS.has(type)) {
		const clean: RichTextNode = { type, content: cleanChildren(node) };
		if (type === "heading") {
			const level =
				typeof attrs.level === "number" && Number.isFinite(attrs.level)
					? attrs.level
					: 2;
			clean.attrs = { level: Math.min(4, Math.max(2, Math.round(level))) };
		}
		return [clean];
	}
	// Unknown nodes are dropped but their children are kept, like the server.
	return cleanChildren(node);
}

// Reduces an editor document to exactly what the server allowlist keeps:
// no extra attrs (colspan, target, rel, ...), no unknown nodes or marks.
export function sanitizeDoc(doc: unknown): RichTextDoc {
	return { type: "doc", content: cleanChildren(doc) };
}

export function isEmptyDoc(doc: RichTextDoc): boolean {
	const hasContent = (node: RichTextNode): boolean =>
		node.type === "image" ||
		node.type === "youtube" ||
		node.type === "table" ||
		(node.type === "text" && (node.text ?? "").trim() !== "") ||
		(node.content ?? []).some(hasContent);
	return !doc.content.some(hasContent);
}

// The generated type is a generic object; narrow it defensively.
export function asDoc(value: unknown): RichTextDoc | null {
	return isRecord(value) && value.type === "doc" ? sanitizeDoc(value) : null;
}
