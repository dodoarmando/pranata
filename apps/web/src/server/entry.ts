import handler, { createServerEntry } from "@tanstack/react-start/server-entry";
import { createHandler } from "./handler";
import { createHtmlCache } from "./html-cache";

const cache = createHtmlCache({ max: 500, ttlMs: 300_000 });

export default createServerEntry({
	fetch(request, opts) {
		const handle = createHandler({
			cache,
			purgeToken: process.env.PRANATA_PURGE_TOKEN ?? "",
			render: () => Promise.resolve(handler.fetch(request, opts)),
		});
		return handle(request);
	},
});
