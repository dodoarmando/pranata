import type { UserConfig } from "@commitlint/types";

const config: UserConfig = {
	extends: ["@commitlint/config-conventional"],
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
