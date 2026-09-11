import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fixtureEnvironment, paths, seedScenario } from "../setup.ts";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-with-remote-branches.sh" });
test("previews conflicted files within commits and the dirty worktree", async ({
	appWindow,
	electronApp,
	testEnvironment,
}) => {
	test.slow();
	execFileSync(paths.but, ["apply", "branch1"], {
		cwd: path.join(testEnvironment.workdir, "local-clone"),
		env: fixtureEnvironment(testEnvironment),
	});
	await seedScenario(
		"project-with-remote-branches__add-conflicting-base-and-dirty-worktree.sh",
		testEnvironment,
	);
	const clone = path.join(testEnvironment.workdir, "local-clone");
	execFileSync("git", ["fetch", "origin"], {
		cwd: clone,
		env: fixtureEnvironment(testEnvironment),
	});
	const head = () =>
		execFileSync("git", ["rev-parse", "HEAD"], { cwd: clone, encoding: "utf8" }).trim();
	const before = head();
	const dirty = readFileSync(path.join(clone, "a_file"), "utf8");
	await appWindow.reload();
	const warning = appWindow.getByRole("button", { name: "Update will cause conflicts" });
	await expect(warning).toBeVisible();
	await expect(appWindow.getByRole("button", { name: "Pull latest", exact: true })).toBeEnabled();
	await warning.focus();
	await appWindow.keyboard.press("Tab");
	await appWindow.keyboard.press("Shift+Tab");
	await appWindow.keyboard.press("Enter");
	await expect(appWindow.getByRole("dialog", { name: "Base update conflicts" })).toBeVisible();
	await expect(
		appWindow.getByText("These files will conflict after the update", { exact: true }),
	).toBeVisible();
	const dialog = appWindow.getByRole("dialog", { name: "Base update conflicts" });
	const commits = dialog.getByRole("list", { name: "Conflicted files", exact: true });
	await expect(commits).toHaveCount(2);
	for (const commit of await commits.all())
		await expect(commit.getByText("a_file", { exact: true })).toBeVisible();

	await expect(dialog.getByText("a_file", { exact: true })).toHaveCount(3);
	await expect(dialog.getByRole("button")).toHaveCount(1);
	await expect(dialog.getByText("Select a commit", { exact: false })).toHaveCount(0);
	await appWindow.keyboard.press("Escape");
	await expect(appWindow.getByRole("dialog", { name: "Base update conflicts" })).toBeHidden();
	await warning.click();
	await expect(dialog).toBeVisible();
	await dialog.getByRole("button", { name: "Close conflict details" }).click();
	await expect(dialog).toBeHidden();
	// The target row and its docked copy must use the same preview and mutation.
	await expect(
		appWindow.getByRole("button", { name: "Update will cause conflicts", includeHidden: true }),
	).toHaveCount(2);
	expect(head()).toBe(before);
	expect(readFileSync(path.join(clone, "a_file"), "utf8")).toBe(dirty);

	await electronApp.evaluate(({ ipcMain }) => {
		ipcMain.removeHandler("workspaceIntegrateUpstream");
		ipcMain.handle(
			"workspaceIntegrateUpstream",
			() => new Promise((resolve) => setTimeout(resolve, 60_000)),
		);
	});
	await appWindow.getByRole("button", { name: "Pull latest", exact: true }).click();
	const pulling = appWindow.getByRole("button", { name: "Pulling…", includeHidden: true });
	await expect(pulling).toHaveCount(2);
	for (const button of await pulling.all()) await expect(button).toBeDisabled();
	await expect(
		appWindow.getByRole("button", { name: "Update will cause conflicts", includeHidden: true }),
	).toHaveCount(0);
});
