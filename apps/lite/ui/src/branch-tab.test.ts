/** @vitest-environment jsdom */
import { describe, expect, it, vi } from "vitest";

const key = "branch_tab:v1";

describe("branch tab", () => {
	it("keeps the pick in local storage", async () => {
		localStorage.removeItem(key);
		vi.resetModules();
		const { writeBranchTab } = await import("./branch-tab.ts");
		writeBranchTab("diff");
		expect(localStorage.getItem(key)).toBe("diff");
	});

	it("loads the stored pick, so repeating it writes nothing", async () => {
		localStorage.setItem(key, "pr");
		vi.resetModules();
		const { writeBranchTab } = await import("./branch-tab.ts");
		const setItem = vi.spyOn(Storage.prototype, "setItem");
		writeBranchTab("pr");
		expect(setItem).not.toHaveBeenCalled();
		setItem.mockRestore();
	});
});
