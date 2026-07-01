import { applyUpstream, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { dragAndDropByLocator, MOD_KEY, stack } from "../src/util.ts";
import { expect } from "@playwright/test";

test("should move multiple selected commits to a different branch via drag and drop", async ({
	page,
	gitbutler,
}) => {
	test.setTimeout(120_000);

	// branch1 (4 commits, modifies a_file) and branch2 (2 commits, modifies b_file) — independent.
	await gitbutler.runScript("project-with-stacks.sh");
	await applyUpstream(gitbutler, "branch1", "branch2");
	await openWorkspace(page);

	const stack1 = stack(page, "branch1");
	const stack2 = stack(page, "branch2");
	await expect(stack1.getByTestId("commit-row")).toHaveCount(4);
	await expect(stack2.getByTestId("commit-row")).toHaveCount(2);

	const b2Second = stack2.getByTestId("commit-row").filter({ hasText: "branch2: second commit" });
	const b2First = stack2.getByTestId("commit-row").filter({ hasText: "branch2: first commit" });

	await b2Second.click();
	await b2First.click({ modifiers: [MOD_KEY] });
	await expect(b2Second).toHaveClass(/\bselected\b/);
	await expect(b2First).toHaveClass(/\bselected\b/);

	// Drag the selected commits onto branch1's header.
	const branch1Header = stack1.getByTestId("branch-header").first();
	await dragAndDropByLocator(page, b2Second, branch1Header, { force: true });

	await expect(stack1.getByTestId("commit-row")).toHaveCount(6, { timeout: 15_000 });
	await expect(
		stack1.getByTestId("commit-row").filter({ hasText: "branch2: first commit" }),
	).toHaveCount(1);
	await expect(
		stack1.getByTestId("commit-row").filter({ hasText: "branch2: second commit" }),
	).toHaveCount(1);
});
