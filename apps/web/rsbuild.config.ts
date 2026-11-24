import { defineConfig } from "@rsbuild/core";
import { pluginReact } from "@rsbuild/plugin-react";
import { tanstackStart } from "@tanstack/react-start/plugin/rsbuild";
import { mediaMiddleware } from "./dev-media";

// Dev only: where the API writes Media Asset variants (`just dev` sets it).
const mediaDir = process.env.PRANATA_MEDIA_DIR;

export default defineConfig({
	server: {
		port: 3000,
		// Dev only: forward API calls to the Rust API.
		proxy: { "/api": process.env.PRANATA_API_URL ?? "http://127.0.0.1:8080" },
		// Dev only: serve /media like Caddy does on an Installation.
		setup: ({ action, server }) => {
			if (action === "dev" && mediaDir) {
				server.middlewares.use("/media", mediaMiddleware(`${mediaDir}/public`));
			}
		},
	},
	resolve: { alias: { "@": "./src" } },
	plugins: [
		pluginReact(),
		tanstackStart({ server: { entry: "./server/entry.ts" } }),
	],
});
