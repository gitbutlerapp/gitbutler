import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import path from "node:path";
import type { ElectronApplication, Page } from "@playwright/test";
import { fixtureEnvironment } from "../setup.ts";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

/** Answers the next native menu by choosing the item labelled `label`. */
const chooseFromNextMenu = (electronApp: ElectronApplication, label: string) =>
	electronApp.evaluate(({ Menu }, label) => {
		Menu.prototype.popup = function (options) {
			this.items.find((item) => item.label === label)?.click();
			options?.callback?.();
		};
	}, label);

const filesToggle = (appWindow: Page) => appWindow.getByRole("button", { name: /^Toggle files/ });

test("the commit filter narrows the branch diff to the ticked commits", async ({
	appWindow,
	electronApp,
	testEnvironment,
}) => {
	// C gets a second commit, so the branch has two to choose between.
	const clone = path.join(testEnvironment.workdir, "local-clone");
	writeFileSync(path.join(clone, "C2.txt"), "C2\n");
	const env = fixtureEnvironment(testEnvironment);
	execFileSync("git", ["-C", clone, "add", "C2.txt"], { env });
	execFileSync("git", ["-C", clone, "commit", "-m", "C: second commit"], { env });
	await appWindow.reload();

	await appWindow
		.getByRole("treeitem", { name: "C", exact: true })
		.getByTitle("C", { exact: true })
		.click();
	const filter = appWindow.getByRole("button", { name: "All 2 commits", exact: true });
	await expect(filter).toBeVisible();
	await expect(filesToggle(appWindow)).toHaveAccessibleName(/2 files changed/);

	await chooseFromNextMenu(electronApp, "C: first commit");
	await filter.click();
	await expect(appWindow.getByRole("button", { name: "1 commit", exact: true })).toBeVisible();
	await expect(filesToggle(appWindow)).toHaveAccessibleName(/1 file changed/);

	await chooseFromNextMenu(electronApp, "All 2 commits");
	await appWindow.getByRole("button", { name: "1 commit", exact: true }).click();
	await expect(filter).toBeVisible();
	await expect(filesToggle(appWindow)).toHaveAccessibleName(/2 files changed/);
});
