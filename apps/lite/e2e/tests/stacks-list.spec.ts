import { expect, test } from "../test.ts";

/** Kept in sync with the argument the scenario script defaults to. */
const stackCount = 12;

test.describe("stacks list", () => {
	test.use({ scenario: "project-with-many-independent-stacks.sh" });

	// The selected stack stays mounted wherever the list is scrolled to, so with
	// it selected and scrolled away from, every other stack's commit virtualizer
	// used to stamp the offset it last remembered back onto the shared scroller.
	test("scrolls to the last stack while the first one is selected", async ({ app }) => {
		// Seeding a workspace this size costs more than the default budget on a loaded CI runner.
		test.slow();
		const stacks = app.getByRole("treeitem", { name: /^stack-\d+$/ });
		const topStack = await stacks.first().getAttribute("aria-label");
		if (topStack === null) throw new Error("Workspace shows no stacks");
		// Which end of the range the list starts at is the workspace's business.
		const bottomStack = topStack === "stack-1" ? `stack-${stackCount}` : "stack-1";
		await app.selectBranch(topStack);
		// The stacks have to outgrow their panel for any of this to mean anything.
		expect((await app.listScroll(app.branch(topStack))).end).toBeGreaterThan(0);

		await app.wheelThroughList(app.branchName(topStack));

		await expect.poll(() => app.isScrolledToEnd(stacks.first())).toBe(true);
		await expect(app.branch(bottomStack)).toBeInViewport();
	});
});
