import { defineConfig } from "@rsbuild/core";
import { pluginReact } from "@rsbuild/plugin-react";
import { tanstackStart } from "@tanstack/react-start/plugin/rsbuild";

export default defineConfig({
	server: {
		port: 3000,
		// Dev only: forward API calls to the Rust API.
		proxy: { "/api": process.env.PRANATA_API_URL ?? "http://127.0.0.1:8080" },
	},
	resolve: { alias: { "@": "./src" } },
	plugins: [pluginReact(), tanstackStart()],
});
