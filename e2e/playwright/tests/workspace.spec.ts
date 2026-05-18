import { assertBranch } from "../src/branch.ts";
import { openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { waitForTestId, waitForTestIdToNotExist } from "../src/util.ts";

test("can switch back to workspace", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	const localClone = gitbutler.pathInWorkdir("local-clone");
	await assertBranch("gitbutler/workspace", localClone);

	await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);
	assertBranch("master", localClone);

	const switchButton = await waitForTestId(page, "chrome-header-switch-back-to-workspace-button");
	await switchButton.click();

	await assertBranch("gitbutler/workspace", localClone);
	await waitForTestIdToNotExist(page, "chrome-header-switch-back-to-workspace-button");
});
