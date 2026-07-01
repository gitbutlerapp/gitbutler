import { applyUpstream, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { dragAndDropByLocator, stack, waitForTestId } from "../src/util.ts";
import { expect } from "@playwright/test";

test("move branch to top of other stack and tear it off", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-stacks.sh");
	await applyUpstream(gitbutler, "branch1", "branch2");
	await openWorkspace(page);

	const branchHeaders = page.getByTestId("branch-header");
	await expect(stack(page)).toHaveCount(2);

	// Drag branch1 to the top of branch2's stack. The dropzone above branch2
	// (isFirst=true) activates on hover during drag — we hit it via position offset.
	await dragAndDropByLocator(
		page,
		branchHeaders.filter({ hasText: "branch1" }),
		branchHeaders.filter({ hasText: "branch2" }),
		{ force: true, position: { x: 120, y: -10 } },
	);

	await expect(stack(page)).toHaveCount(1);
	await expect(branchHeaders).toHaveCount(2);

	// Now tear off branch2 onto the new stack dropzone.
	const stackDropzone = await waitForTestId(page, "stack-offlane-dropzone");
	await dragAndDropByLocator(page, branchHeaders.filter({ hasText: "branch2" }), stackDropzone, {
		force: true,
		position: { x: 10, y: 10 },
	});

	await expect(stack(page)).toHaveCount(2);
	await expect(branchHeaders).toHaveCount(2);
});

test("move branch to the middle of other stack", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-stacks.sh");
	await applyUpstream(gitbutler, "branch1", "branch2", "branch3");
	await openWorkspace(page);

	const branchHeaders = page.getByTestId("branch-header");
	await expect(stack(page)).toHaveCount(3);

	// Move branch2 on top of branch1.
	await dragAndDropByLocator(
		page,
		branchHeaders.filter({ hasText: "branch2" }),
		branchHeaders.filter({ hasText: "branch1" }),
		{ force: true, position: { x: 120, y: -10 } },
	);
	await expect(stack(page)).toHaveCount(2);

	// Move branch3 on top of branch1 (now in the middle of the merged stack).
	await dragAndDropByLocator(
		page,
		branchHeaders.filter({ hasText: "branch3" }),
		branchHeaders.filter({ hasText: "branch1" }),
		{ force: true, position: { x: 120, y: 0 } },
	);
	await expect(stack(page)).toHaveCount(1);
	await expect(branchHeaders).toHaveCount(3);
});
