import {
	BranchDropData,
	acceptsSameStackBranchDrop,
	moveBranchPlacement,
} from "$lib/dragging/dropHandlers/branchDropHandler";
import { describe, expect, test } from "vitest";

function branchDropData(branchName: string, numberOfCommits: number): BranchDropData {
	return new BranchDropData("stack-1", branchName, false, 2, numberOfCommits, undefined, []);
}

describe("acceptsSameStackBranchDrop", () => {
	test("accepts empty branch reorders", () => {
		const data = branchDropData("source", 0);

		expect(acceptsSameStackBranchDrop(data, "target")).toBe(true);
	});

	test("accepts non-empty branch reorders", () => {
		const data = branchDropData("source", 1);

		expect(acceptsSameStackBranchDrop(data, "target")).toBe(true);
	});

	test("rejects self moves", () => {
		const data = branchDropData("source", 1);

		expect(acceptsSameStackBranchDrop(data, "source")).toBe(false);
	});
});

describe("moveBranchPlacement", () => {
	test("goes above the target branch's reference by default", () => {
		expect(moveBranchPlacement("refs/heads/feature/target")).toEqual({
			relativeTo: { type: "reference", subject: "refs/heads/feature/target" },
			side: "above",
		});
	});

	test("goes below the target branch's last commit for the zone under it", () => {
		expect(moveBranchPlacement("refs/heads/feature/target", "abc123")).toEqual({
			relativeTo: { type: "commit", subject: "abc123" },
			side: "below",
		});
	});
});
