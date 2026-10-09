import { execFileSync } from "node:child_process";
import path from "node:path";
import { processEnvironment, type LiteTestEnvironment } from "../setup.ts";
import { expect, test } from "../test.ts";

/**
 * Move HEAD from outside the app, the way a terminal `git checkout` does.
 *
 * The app is told about this by the watcher rather than by its own mutations,
 * so nothing here touches the UI: the assertions are about what the workspace
 * shows once the events have landed.
 */
const git = (environment: LiteTestEnvironment, ...args: Array<string>): void => {
	execFileSync("git", args, {
		cwd: path.join(environment.workdir, "local-clone"),
		env: processEnvironment({ GIT_CONFIG_GLOBAL: environment.gitConfig }),
		stdio: "pipe",
	});
};

const checkout = (environment: LiteTestEnvironment, ...args: Array<string>): void =>
	git(environment, "checkout", ...args);

test.describe("head moves", () => {
	// Ends on `gitbutler/workspace` with one applied lane, so leaving the
	// workspace and coming back are both visible in the stacks list.
	test.use({ scenario: "project-with-editable-commit.sh" });

	test("refreshes the workspace when HEAD moves outside the app", async ({
		appWindow,
		testEnvironment,
	}) => {
		const lane = appWindow.getByRole("treeitem", { name: "edit-branch", exact: true });
		const master = appWindow.getByRole("treeitem", { name: "master", exact: true });

		// In the workspace: the applied lane is what the sidebar lists.
		await expect(lane).toBeVisible();
		await expect(master).toHaveCount(0);

		// Onto a branch: the workspace is left behind, and the checked-out branch
		// is all there is to show.
		checkout(testEnvironment, "master");
		await expect(lane).toHaveCount(0);
		await expect(master).toBeVisible();

		// Detached: no branch at HEAD, so no branch to list.
		checkout(testEnvironment, "--detach");
		await expect(master).toHaveCount(0);
		await expect(lane).toHaveCount(0);

		// Back onto the workspace ref: the lane returns without the app being
		// told anything other than that HEAD moved.
		checkout(testEnvironment, "gitbutler/workspace");
		await expect(lane).toBeVisible();
		await expect(master).toHaveCount(0);
	});

	test("refreshes the workspace when only HEAD itself is rewritten", async ({
		appWindow,
		testEnvironment,
	}) => {
		const lane = appWindow.getByRole("treeitem", { name: "edit-branch", exact: true });
		const master = appWindow.getByRole("treeitem", { name: "master", exact: true });

		await expect(lane).toBeVisible();

		// Not a checkout: this rewrites `.git/HEAD` and leaves the worktree, the
		// index and the reflog alone. The workspace has to follow HEAD however it
		// moved, not only when a checkout moves it.
		git(testEnvironment, "symbolic-ref", "HEAD", "refs/heads/master");
		await expect(lane).toHaveCount(0);
		await expect(master).toBeVisible();
	});
});

test.describe("head moves between branches", () => {
	// Ends on `C` with `A` and `B` behind it as local branches. Moving between
	// them leaves the operating mode reading `OutsideWorkspace` throughout, so
	// only the branch it names changes.
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	test("refreshes when HEAD moves between branches without changing the mode", async ({
		appWindow,
		testEnvironment,
	}) => {
		const top = appWindow.getByRole("treeitem", { name: "C", exact: true });
		const bottom = appWindow.getByRole("treeitem", { name: "A", exact: true });
		const middle = appWindow.getByRole("treeitem", { name: "B", exact: true });

		// On the tip of the stack, all three branches are in view.
		await expect(top).toBeVisible();
		await expect(middle).toBeVisible();
		await expect(bottom).toBeVisible();

		// Down to the bottom of the stack: same mode, different branch, so the
		// two above HEAD have to leave the list.
		checkout(testEnvironment, "A");
		await expect(bottom).toBeVisible();
		await expect(top).toHaveCount(0);
		await expect(middle).toHaveCount(0);

		// And back up, to catch a refresh that only works in one direction.
		checkout(testEnvironment, "C");
		await expect(top).toBeVisible();
		await expect(middle).toBeVisible();
		await expect(bottom).toBeVisible();
	});
});
