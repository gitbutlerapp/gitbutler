import { expect, test } from "../test.ts";

/** Kept in sync with the argument the scenario script defaults to. */
const stackCount = 12;

test.describe("stacks list", () => {
	test.use({ scenario: "project-with-many-independent-stacks.sh" });

	// The selected stack stays mounted wherever the list is scrolled to, so with
	// it selected and scrolled away from, every other stack's commit virtualizer
	// used to stamp the offset it last remembered back onto the shared scroller:
	// the list snapped back to the same place on every frame and went no further.
	test("scrolls to the last stack while the first one is selected", async ({ appWindow }) => {
		// Seeding a workspace this size and then driving it by hand costs more than
		// the default budget allows for on a loaded CI runner.
		test.slow();

		await expect(appWindow.getByRole("treeitem", { name: /^stack-\d+$/ }).first()).toBeVisible();

		const stackNames = await appWindow.getByRole("treeitem").evaluateAll((rows) =>
			rows.flatMap((row) => {
				const name = row.getAttribute("aria-label");
				return name !== null && name.startsWith("stack-") ? [name] : [];
			}),
		);

		const topStack = stackNames[0];
		if (topStack === undefined) throw new Error("Workspace shows no stacks");
		// Which end of the range the list starts at is the workspace's business.
		const bottomStack = topStack === "stack-1" ? `stack-${stackCount}` : "stack-1";

		const top = appWindow.getByRole("treeitem", { name: topStack, exact: true });
		// The tree item spans the branch row and the commits under it, so aim at
		// the name rather than the item's middle, which is a commit row.
		await top.getByText(topStack, { exact: true }).click();
		await expect(top).toHaveAttribute("aria-selected", "true");

		const scroll = (): Promise<{ offset: number; end: number }> =>
			appWindow.evaluate(() => {
				// Anchored on a stack row: the hidden sidebar tabs are still in the
				// document, and they look just like this list from the outside.
				const row = document.querySelector('[role="treeitem"][aria-label^="stack-"]');
				const scroller = row?.closest('[role="tree"]')?.parentElement;
				if (!scroller) throw new Error("Stacks list has no scroller");

				return {
					offset: Math.round(scroller.scrollTop),
					end: Math.round(scroller.scrollHeight - scroller.clientHeight),
				};
			});

		// The stacks have to outgrow their panel for any of this to mean anything.
		expect((await scroll()).end).toBeGreaterThan(0);

		const box = await top.boundingBox();
		if (box === null) throw new Error("Stack row has no bounding box");
		await appWindow.mouse.move(box.x + box.width / 2, box.y + box.height / 2);

		// Enough wheel to cross the whole list several times over; the scroller
		// stops at its end. Few, large steps rather than many small ones: each one
		// is a round trip, and a stuck list stays stuck however finely it is asked
		// to move.
		for (let step = 0; step < 16; step++) await appWindow.mouse.wheel(0, 400);

		await expect
			.poll(async () => {
				const { offset, end } = await scroll();
				return offset >= end;
			})
			.toBe(true);
		await expect(
			appWindow.getByRole("treeitem", { name: bottomStack, exact: true }),
		).toBeInViewport();
	});
});
