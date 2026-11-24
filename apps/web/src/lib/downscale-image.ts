const PASS_THROUGH_BYTES = 2 * 1024 * 1024;

// Scales (width, height) so the longest edge is at most `maxEdge`.
export function fitWithin(
	width: number,
	height: number,
	maxEdge: number,
): { width: number; height: number } {
	const longest = Math.max(width, height);
	if (longest <= maxEdge) return { width, height };
	const scale = maxEdge / longest;
	return {
		width: Math.max(1, Math.round(width * scale)),
		height: Math.max(1, Math.round(height * scale)),
	};
}

export function jpegName(name: string): string {
	const dot = name.lastIndexOf(".");
	return `${dot > 0 ? name.slice(0, dot) : name}.jpg`;
}

// Shrinks big phone photos before upload so weak signal does not stall them.
// Anything it cannot or need not touch is passed through for the server to judge.
export async function downscaleImage(
	file: File,
	maxEdge = 2560,
	quality = 0.85,
): Promise<Blob> {
	if (!file.type.startsWith("image/")) return file;
	if (
		typeof OffscreenCanvas === "undefined" ||
		typeof createImageBitmap === "undefined"
	) {
		return file;
	}

	let bitmap: ImageBitmap;
	try {
		bitmap = await createImageBitmap(file, { imageOrientation: "from-image" });
	} catch {
		return file;
	}

	try {
		const { width, height } = fitWithin(bitmap.width, bitmap.height, maxEdge);
		const resized = width !== bitmap.width || height !== bitmap.height;
		if (!resized && file.size < PASS_THROUGH_BYTES) return file;

		const canvas = new OffscreenCanvas(width, height);
		const context = canvas.getContext("2d");
		if (!context) return file;
		context.drawImage(bitmap, 0, 0, width, height);
		const blob = await canvas.convertToBlob({ type: "image/jpeg", quality });
		// Re-encoding without shrinking must not make the upload larger.
		if (!resized && blob.size >= file.size) return file;
		return new File([blob], jpegName(file.name), {
			type: "image/jpeg",
			lastModified: file.lastModified,
		});
	} catch {
		return file;
	} finally {
		bitmap.close();
	}
}
