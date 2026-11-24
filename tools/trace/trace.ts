import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const ID = /D-\d{2,3}/g;
const DEFINITION = /^\*\*(D-\d{2,3}) /;

export type TraceResult = {
	ok: boolean;
	missing: string[];
	tasksWithoutDecisions: string[];
	unknown: string[];
	ranges: string[];
	errors: string[];
};

const RANGE = /…|\.{3}|D-\d+\s*[–—-]\s*D-\d+/;

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
	taskCount: number;
	ranges: string[];
};

function cite(plans: string[]): Citations {
	const result: Citations = {
		task: new Set(),
		notBuilt: new Set(),
		roadmap: new Set(),
		tasksWithoutDecisions: [],
		taskCount: 0,
		ranges: [],
	};
	for (const plan of plans) {
		let pending: string | undefined;
		let current = "no task";
		const closeTask = () => {
			if (pending !== undefined) result.tasksWithoutDecisions.push(pending);
			pending = undefined;
		};
		for (const line of plan.split("\n")) {
			if (line.startsWith("### ")) {
				closeTask();
				const heading = /^### (Task \d+:.*)$/.exec(line);
				if (heading?.[1]) {
					pending = heading[1].trim();
					current = pending;
					result.taskCount++;
				}
			} else if (line.startsWith("**Decisions:**")) {
				if (RANGE.test(line)) {
					result.ranges.push(`${current}: ${line.slice(14).trim()}`);
				}
				for (const id of ids(line)) result.task.add(id);
				pending = undefined;
			} else if (line.startsWith("**Not built:**")) {
				if (RANGE.test(line)) {
					result.ranges.push(`Not built: ${line.slice(14).trim()}`);
				}
				for (const id of ids(line)) result.notBuilt.add(id);
			} else if (/^\|\s*R\d+-[A-Z]\b/.test(line)) {
				const cells = line.split("|").slice(1, -1);
				const last = cells.at(-1) ?? "";
				if (RANGE.test(last)) {
					result.ranges.push(`roadmap: ${(cells[0] ?? "").trim()}`);
				}
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
	const errors: string[] = [];
	if (known.size === 0) errors.push("no decisions found in specs");
	if (cited.taskCount === 0) errors.push("no tasks found in plans");
	return {
		ok:
			missing.length === 0 &&
			unknown.length === 0 &&
			tasksWithoutDecisions.length === 0 &&
			cited.ranges.length === 0 &&
			errors.length === 0,
		missing,
		tasksWithoutDecisions,
		unknown,
		ranges: cited.ranges,
		errors,
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
	for (const error of result.errors) lines.push(`Error: ${error}`);
	if (result.ranges.length > 0) {
		lines.push("Ranges are not allowed, list each ID individually:");
		for (const range of result.ranges) lines.push(`  - ${range}`);
	}
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
	let result: TraceResult;
	try {
		result = check(
			readMarkdown(join(root, "docs/superpowers/specs")),
			readMarkdown(join(root, "docs/superpowers/plans")),
			strict,
		);
	} catch (error) {
		const reason = error instanceof Error ? error.message : String(error);
		console.error(`trace: cannot read docs directories: ${reason}`);
		process.exit(1);
	}
	console.log(report(result, strict));
	process.exit(result.ok ? 0 : 1);
}
