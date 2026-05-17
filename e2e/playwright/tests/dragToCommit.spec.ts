import { updateCommitMessage, verifyCommitMessageEditor } from "../src/commit.ts";
import {
	assertFileContent,
	assertFileIsStaged,
	assertFileIsUncommitted,
	assertFileIsUnstaged,
} from "../src/file.ts";
import { applyUpstream, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, commitRow, dragAndDropByLocator, waitForTestId } from "../src/util.ts";
import { expect } from "@playwright/test";
import { writeFileSync } from "fs";

test("should be able to start a commit by dragging a file", async ({ page, gitbutler }) => {
	const fileName = "b_file";
	const filePath = gitbutler.pathInWorkdir("local-clone/" + fileName);
	const fileContent = "Hello! this is file b\n";
	const anotherFileName = "c_file";
	const anotherFilePath = gitbutler.pathInWorkdir("local-clone/" + anotherFileName);
	const anotherFileContent = "Hello! this is file c\n";

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	writeFileSync(filePath, fileContent, { flag: "w" });
	writeFileSync(anotherFilePath, anotherFileContent, { flag: "w" });

	const fileLocator = page
		.getByTestId("uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
	await expect(fileLocator).toBeVisible();

	const branchCard = (await waitForTestId(page, "branch-card")).filter({ hasText: "branch1" });
	await dragAndDropByLocator(page, fileLocator, branchCard);

	await assertFileIsStaged(page, fileName);
	await assertFileIsUnstaged(page, anotherFileName);

	const commitTitle = "New file added";
	const commitMessage = `Added ${fileName} to the project`;
	await verifyCommitMessageEditor(page, "", "");
	await updateCommitMessage(page, commitTitle, commitMessage);
	await clickByTestId(page, "commit-drawer-action-button");

	await expect(commitRow(page, commitTitle)).toHaveCount(1);
	await assertFileIsUncommitted(page, anotherFileName);
	await assertFileContent(filePath, fileContent);
	await assertFileContent(anotherFilePath, anotherFileContent);
});
