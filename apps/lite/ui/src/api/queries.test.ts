import { listCIChecksQueryOptions } from "#ui/api/queries.ts";
import type { CiCheck } from "@gitbutler/but-sdk";
import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

const check = (status: CiCheck["status"]): CiCheck =>
	({ name: "ci", status }) as unknown as CiCheck;
const running = check("inProgress");
const passed = check({ complete: { conclusion: "success", completed_at: null } });

/** Runs the checks query once per response, on the same client, and lists what it invalidated. */
const pollChecks = async (responses: Array<Array<CiCheck>>) => {
	const client = new QueryClient();
	const invalidated = vi.spyOn(client, "invalidateQueries").mockResolvedValue();
	const listCiChecks = vi.fn();
	vi.stubGlobal("window", { lite: { listCiChecks } });
	const options = listCIChecksQueryOptions({
		projectId: "p1",
		reference: "feature",
		polling: "priority",
	});
	for (const response of responses) {
		listCiChecks.mockResolvedValueOnce(response);
		await client.fetchQuery({ ...options, staleTime: 0 });
	}
	return invalidated.mock.calls.map(([filters]) => filters?.queryKey);
};

describe("listCIChecksQueryOptions", () => {
	it("refetches the merge status once the checks reach a verdict", async () => {
		expect(await pollChecks([[running], [running], [passed]])).toEqual([
			["p1", "getReviewMergeStatus"],
		]);
	});

	it("leaves the merge status alone while checks are still running or already settled", async () => {
		expect(await pollChecks([[running], [running]])).toEqual([]);
		expect(await pollChecks([[passed], [passed]])).toEqual([]);
		expect(await pollChecks([[passed], [running]])).toEqual([]);
	});
});
