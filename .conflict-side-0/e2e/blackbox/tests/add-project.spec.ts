import { spawnAndLog, findAndClick, findElement, setElementValue } from "../utils.js";

describe("Project", () => {
	before(() => {
		spawnAndLog("bash", ["-c", "./blackbox/scripts/init-repositories.sh ../target/debug/but"]);
	});

	it("should add a local project", async () => {
		await findAndClick('button[data-testid="analytics-continue"]');

		const dirInput = await findElement('input[data-testid="test-directory-path"]');
		await setElementValue(dirInput, "/tmp/gb-e2e-repos/one-vbranch-on-integration");

		await findAndClick('button[data-testid="add-local-project"]');
		// TODO: Remove next click when v3 is default!
		await findAndClick('button[data-testid="set-base-branch"]');
		await findElement('button[data-testid="navigation-workspace-button"]');
	});
});
