import { applyUpstream, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, commitRow, waitForTestId } from "../src/util.ts";
import { expect } from "@playwright/test";
import { readFileSync, writeFileSync } from "fs";

test("should be able to edit a commit through the edit mode", async ({ page, gitbutler }) => {
	const fileBPath = gitbutler.pathInWorkdir("file-b.txt");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	const bottomCommit = commitRow(page, "branch1: first commit");
	await bottomCommit.click({ button: "right" });
	await clickByTestId(page, "commit-row-context-menu-edit-commit");
	await waitForTestId(page, "edit-mode");

	writeFileSync(fileBPath, "This is file B\n", { encoding: "utf-8" });

	await clickByTestId(page, "edit-mode-save-and-exit-button");
	await waitForTestId(page, "workspace-view");

	expect(readFileSync(fileBPath, { encoding: "utf-8" })).toEqual("This is file B\n");
});
