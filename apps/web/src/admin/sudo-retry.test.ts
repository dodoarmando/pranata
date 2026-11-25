import { describe, expect, it, rstest } from "@rstest/core";
import { ApiError } from "@/api/fetcher";
import { withSudo } from "./sudo-retry";

function problem(code: string, status = 403) {
	return new ApiError(status, {
		type: "about:blank",
		title: code,
		status,
		code,
	} as never);
}

describe("withSudo", () => {
	it("retries the request once after confirming", async () => {
		const run = rstest
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(problem("sudo_required"))
			.mockResolvedValueOnce("ok");
		const confirm = rstest.fn(async () => true);
		await expect(withSudo(run, confirm)).resolves.toBe("ok");
		expect(run).toHaveBeenCalledTimes(2);
		expect(confirm).toHaveBeenCalledTimes(1);
	});

	it("rethrows when the user cancels", async () => {
		const error = problem("sudo_required");
		const run = rstest.fn<() => Promise<string>>().mockRejectedValue(error);
		await expect(withSudo(run, async () => false)).rejects.toBe(error);
		expect(run).toHaveBeenCalledTimes(1);
	});

	it("does not loop", async () => {
		const run = rstest
			.fn<() => Promise<string>>()
			.mockRejectedValue(problem("sudo_required"));
		await expect(withSudo(run, async () => true)).rejects.toBeInstanceOf(
			ApiError,
		);
		expect(run).toHaveBeenCalledTimes(2);
	});

	it("passes other errors through", async () => {
		const error = problem("permission_denied");
		const run = rstest.fn<() => Promise<string>>().mockRejectedValue(error);
		const confirm = rstest.fn(async () => true);
		await expect(withSudo(run, confirm)).rejects.toBe(error);
		expect(confirm).not.toHaveBeenCalled();
	});
});
