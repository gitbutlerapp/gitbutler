/** @vitest-environment jsdom */

import { beforeEach, describe, expect, it } from "vitest";
import {
	createInitialBranchesState,
	readStoredBranchFilters,
	writeStoredBranchFilters,
} from "./branches.ts";

describe("branch filters", () => {
	beforeEach(() => window.localStorage.clear());

	it("start with nothing switched on", () => {
		expect(createInitialBranchesState().filters).toEqual({
			showEmpty: false,
			onlyLocal: false,
			onlyStacks: false,
		});
	});

	it("are stored per project", () => {
		writeStoredBranchFilters("one", { showEmpty: true, onlyLocal: false, onlyStacks: false });
		writeStoredBranchFilters("two", { showEmpty: false, onlyLocal: true, onlyStacks: true });

		expect(readStoredBranchFilters()).toEqual({
			one: { showEmpty: true, onlyLocal: false, onlyStacks: false },
			two: { showEmpty: false, onlyLocal: true, onlyStacks: true },
		});
	});

	it("ignore entries that are not filters, keeping the rest", () => {
		writeStoredBranchFilters("good", { showEmpty: false, onlyLocal: true, onlyStacks: false });
		window.localStorage.setItem("branch_filters:v1:junk", "{not json");
		window.localStorage.setItem("branch_filters:v1:partial", JSON.stringify({ onlyLocal: true }));
		window.localStorage.setItem(
			"branch_filters:v1:typed",
			JSON.stringify({ showEmpty: "yes", onlyLocal: true, onlyStacks: false }),
		);
		window.localStorage.setItem("pr_activity_seen:v1:good", "{}");

		expect(readStoredBranchFilters()).toEqual({
			good: { showEmpty: false, onlyLocal: true, onlyStacks: false },
		});
	});

	it("seed a project's initial state", () => {
		const filters = { showEmpty: true, onlyLocal: true, onlyStacks: false };
		expect(createInitialBranchesState(filters)).toEqual({ filters, search: null, unfolded: {} });
	});
});
