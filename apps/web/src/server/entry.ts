import handler, { createServerEntry } from "@tanstack/react-start/server-entry";
import { createHandler } from "./handler";
import { createHtmlCache } from "./html-cache";

type StartOptions = Parameters<typeof handler.fetch>[1];

// One handler for the process lifetime: the cache and single-flight map are shared.
const handle = createHandler<StartOptions>({
	cache: createHtmlCache({ max: 500, ttlMs: 300_000 }),
	purgeToken: process.env.PRANATA_PURGE_TOKEN ?? "",
	render: async (request, opts) => handler.fetch(request, opts),
});

export default createServerEntry({
	fetch(request, opts) {
		return handle(request, opts);
	},
});
