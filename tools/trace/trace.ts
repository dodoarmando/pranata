import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const ID = /D-\d{2,3}/g;
const DEFINITION = /^\*\*(D-\d{2,3}) /;

export type TraceResult = {
	ok: boolean;
	missing: string[];
	tasksWithoutDecisions: string[];
	unknown: string[];
};

function ids(text: string): string[] {
	return text.match(ID) ?? [];
}

function defined(specs: string[]): Set<string> {
	const found = new Set<string>();
	for (const spec of specs) {
		for (const line of spec.split("\n")) {
			const match = DEFINITION.exec(line);
			if (match?.[1]) found.add(match[1]);
		}
	}
	return found;
}

type Citations = {
	task: Set<string>;
	notBuilt: Set<string>;
	roadmap: Set<string>;
	tasksWithoutDecisions: string[];
};

function cite(plans: string[]): Citations {
	const result: Citations = {
		task: new Set(),
		notBuilt: new Set(),
		roadmap: new Set(),
		tasksWithoutDecisions: [],
	};
	for (const plan of plans) {
		let pending: string | undefined;
		const closeTask = () => {
			if (pending !== undefined) result.tasksWithoutDecisions.push(pending);
			pending = undefined;
		};
		for (const line of plan.split("\n")) {
			if (line.startsWith("### ")) {
				closeTask();
				const heading = /^### (Task \d+:.*)$/.exec(line);
				if (heading?.[1]) pending = heading[1].trim();
			} else if (line.startsWith("**Decisions:**")) {
				for (const id of ids(line)) result.task.add(id);
				pending = undefined;
			} else if (line.startsWith("**Not built:**")) {
				for (const id of ids(line)) result.notBuilt.add(id);
			} else if (/^\|\s*P\d+\b/.test(line)) {
				const cells = line.split("|").slice(1, -1);
				const last = cells.at(-1) ?? "";
				for (const id of ids(last)) result.roadmap.add(id);
			}
		}
		closeTask();
	}
	return result;
}

export function check(
	specs: string[],
	plans: string[],
	strict: boolean,
): TraceResult {
	const known = defined(specs);
	const cited = cite(plans);
	const accepted = new Set([...cited.task, ...cited.notBuilt]);
	if (!strict) for (const id of cited.roadmap) accepted.add(id);

	const all = new Set([...cited.task, ...cited.notBuilt, ...cited.roadmap]);
	const missing = [...known].filter((id) => !accepted.has(id)).sort();
	const unknown = [...all].filter((id) => !known.has(id)).sort();
	const tasksWithoutDecisions = cited.tasksWithoutDecisions;
	return {
		ok:
			missing.length === 0 &&
			unknown.length === 0 &&
			tasksWithoutDecisions.length === 0,
		missing,
		tasksWithoutDecisions,
		unknown,
	};
}

function readMarkdown(dir: string): string[] {
	return readdirSync(dir)
		.filter((name) => name.endsWith(".md"))
		.sort()
		.map((name) => readFileSync(join(dir, name), "utf8"));
}

export function report(result: TraceResult, strict: boolean): string {
	const mode = strict ? "strict" : "default";
	if (result.ok) return `trace (${mode}): ok`;
	const lines = [`trace (${mode}): FAILED`];
	if (result.missing.length > 0) {
		lines.push(
			`Decisions not cited by any plan task: ${result.missing.join(", ")}`,
		);
	}
	if (result.tasksWithoutDecisions.length > 0) {
		lines.push("Tasks without a Decisions line:");
		for (const task of result.tasksWithoutDecisions) lines.push(`  - ${task}`);
	}
	if (result.unknown.length > 0) {
		lines.push(
			`Cited but not defined in any spec: ${result.unknown.join(", ")}`,
		);
	}
	return lines.join("\n");
}

if (import.meta.main) {
	const strict = process.argv.includes("--strict");
	const root = resolve(import.meta.dir, "../..");
	const result = check(
		readMarkdown(join(root, "docs/superpowers/specs")),
		readMarkdown(join(root, "docs/superpowers/plans")),
		strict,
	);
	console.log(report(result, strict));
	process.exit(result.ok ? 0 : 1);
}
