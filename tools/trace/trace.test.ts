import { describe, expect, test } from "bun:test";
import { check } from "./trace";

const spec = "**D-01 First.** text\n\n**D-02 Second.** text\n";

describe("check", () => {
	test("reports a decision cited nowhere", () => {
		const plan = "### Task 1: Foo\n\n**Decisions:** D-01\n";
		const result = check([spec], [plan], false);
		expect(result.missing).toEqual(["D-02"]);
		expect(result.ok).toBe(false);
	});

	test("accepts roadmap citations in default mode", () => {
		const plan = [
			"### Task 1: Foo",
			"**Decisions:** D-01",
			"| Plan | Scope | Decisions |",
			"| --- | --- | --- |",
			"| P2 Access | Roles | D-02 |",
		].join("\n");
		const result = check([spec], [plan], false);
		expect(result.missing).toEqual([]);
		expect(result.ok).toBe(true);
	});

	test("rejects roadmap-only citations in strict mode", () => {
		const plan = [
			"### Task 1: Foo",
			"**Decisions:** D-01",
			"| P2 Access | Roles | D-02 |",
		].join("\n");
		const result = check([spec], [plan], true);
		expect(result.missing).toEqual(["D-02"]);
		expect(result.ok).toBe(false);
	});

	test("accepts Not built citations in both modes", () => {
		const plan = [
			"### Task 1: Foo",
			"**Decisions:** D-01",
			"**Not built:** D-02 (non-goal).",
		].join("\n");
		expect(check([spec], [plan], false).ok).toBe(true);
		expect(check([spec], [plan], true).ok).toBe(true);
	});

	test("reports a task heading without a Decisions line", () => {
		const plan = [
			"### Task 1: Foo",
			"**Decisions:** D-01, D-02",
			"### Task 3: Foo",
			"Some body.",
			"### Task 4: Bar",
			"**Decisions:** D-01",
		].join("\n");
		const result = check([spec], [plan], false);
		expect(result.tasksWithoutDecisions).toEqual(["Task 3: Foo"]);
		expect(result.ok).toBe(false);
	});

	test("reports unknown ids", () => {
		const plan = "### Task 1: Foo\n**Decisions:** D-01, D-02, D-99\n";
		const result = check([spec], [plan], false);
		expect(result.unknown).toEqual(["D-99"]);
		expect(result.ok).toBe(false);
	});

	test("rejects decision ranges", () => {
		for (const range of [
			"D-01…D-02",
			"D-01...D-02",
			"D-01 - D-02",
			"D-01–D-02",
		]) {
			const plan = `### Task 1: Foo\n**Decisions:** ${range}\n`;
			const result = check([spec], [plan], false);
			expect(result.ranges).toEqual([`Task 1: Foo: ${range}`]);
			expect(result.ok).toBe(false);
		}
		const roadmap =
			"### Task 1: Foo\n**Decisions:** D-01, D-02\n| P2 Access | Roles | D-01…D-02 |\n";
		const result = check([spec], [roadmap], false);
		expect(result.ranges).toEqual(["roadmap: P2 Access"]);
		expect(result.ok).toBe(false);
	});

	test("fails when specs define no decisions", () => {
		const plan = "### Task 1: Foo\n**Decisions:** D-01\n";
		const result = check(["no headings here"], [plan], false);
		expect(result.errors).toEqual(["no decisions found in specs"]);
		expect(result.ok).toBe(false);
	});

	test("fails when plans contain no tasks", () => {
		const result = check([spec], ["nothing"], false);
		expect(result.errors).toContain("no tasks found in plans");
		expect(result.ok).toBe(false);
	});
});
