import { assertFileContent, writeToFile } from "../src/file.ts";
import { applyUpstream, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, waitForTestId } from "../src/util.ts";
import { expect } from "@playwright/test";

test("should be able to absorb a file to a commit - simple", async ({ page, gitbutler }) => {
	const filePath = gitbutler.pathInWorkdir("local-clone/a_file");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	const newFileContent = `This is the NEW content of the file.\n`;
	// Create a locked change by replacing the file content completely
	writeToFile(filePath, newFileContent);

	const fileRow = (await waitForTestId(page, "file-list-item")).filter({ hasText: "a_file" });
	await fileRow.click({ button: "right" });
	await clickByTestId(page, "file-list-item-context-menu__absorb");

	await waitForTestId(page, "absorb-modal");
	const commitAbsorptionItems = await waitForTestId(page, "absorb-modal-commit-absorption");
	await expect(commitAbsorptionItems).toHaveCount(1);

	await clickByTestId(page, "absorb-modal-action-button");

	await assertFileContent(filePath, newFileContent);

	const files = await waitForTestId(page, "file-list-item");
	await expect(files).toHaveCount(0);
});
