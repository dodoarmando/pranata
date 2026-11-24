import type { UserConfig } from "@commitlint/types";

const config: UserConfig = {
	extends: ["@commitlint/config-conventional"],
	// release-please's release commit (see docs/release.md) is written by a bot
	// and carries no sign-off. Only its exact title is skipped: "chore(release):"
	// followed by a semantic version and nothing else on the first line.
	ignores: [
		(message) =>
			/^chore\(release\): \d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\r?\n|$)/.test(
				message,
			),
	],
	plugins: [
		{
			rules: {
				"signed-off-by": ({ raw }) => {
					const ok = /^Signed-off-by: .+ <.+>$/m.test(raw ?? "");
					return [
						ok,
						"commit message must contain a Signed-off-by trailer (use git commit -s)",
					];
				},
			},
		},
	],
	rules: {
		"scope-enum": [
			2,
			"always",
			[
				"api",
				"web",
				"infra",
				"ci",
				"tools",
				"deps",
				"adr",
				"spec",
				"plan",
				"release",
			],
		],
		"signed-off-by": [2, "always"],
	},
};

export default config;
