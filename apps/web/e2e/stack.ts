import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "../../..");

export const ORIGIN = `https://localhost:${process.env.PRANATA_E2E_PORT ?? "8443"}`;
export const NOINIT_ORIGIN = `https://localhost:${process.env.PRANATA_E2E_NOINIT_PORT ?? "8444"}`;

export const ADMIN_USERNAME = "e2e";
export const ADMIN_PASSWORD = "e2e-password-123";
// Written by the admin spec, read back by the public spec.
export const OVERVIEW_TEXT = "Ringkasan desa dari pengujian ujung ke ujung.";
export const VILLAGE_HEADING = "Desa Sorek II";

// The Access Link is the full /akses#token URL written by stack.sh.
export function accessLink(): string {
	return readFileSync(
		resolve(repoRoot, "target/e2e/access-link.txt"),
		"utf8",
	).trim();
}

export function stackScript(action: "up" | "down", noInit = false): void {
	const args = [resolve(repoRoot, "infra/dev/stack.sh"), action];
	if (noInit) args.push("--no-init");
	execFileSync("bash", args, { stdio: "inherit" });
}
