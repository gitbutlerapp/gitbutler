import { openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, getByTestId } from "../src/util.ts";
import { expect, type Page, type Request } from "@playwright/test";
import { execFileSync } from "node:child_process";

function git(pathToRepo: string, args: string[]): string {
	return execFileSync("git", args, {
		cwd: pathToRepo,
		encoding: "utf8",
	}).trim();
}

async function workspaceFetchRequests(page: Page, action?: string): Promise<Request> {
	return await page.waitForRequest((request) => {
		if (!request.url().endsWith("/workspace_fetch_from_remotes")) return false;
		if (!action) return true;
		return request.postDataJSON()?.action === action;
	});
}

test("auto-fetch sends auto as the workspace fetch action", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");

	const fetchRequest = workspaceFetchRequests(page, "auto");
	await openWorkspace(page);

	expect((await fetchRequest).postDataJSON()).toMatchObject({
		action: "auto",
	});
});

test.describe("manual workspace fetch", () => {
	test.use({
		gitbutlerOptions: {
			config: {
				onboardingComplete: true,
				fetch: { autoFetchIntervalMinutes: -1 },
			},
		},
	});

	test("sync button sends modal as the workspace fetch action", async ({ page, gitbutler }) => {
		await gitbutler.runScript("project-with-remote-branches.sh");
		await openWorkspace(page);

		const fetchRequest = workspaceFetchRequests(page, "modal");
		await clickByTestId(page, "sync-button");

		expect((await fetchRequest).postDataJSON()).toMatchObject({
			action: "modal",
		});
	});

	test("sync button shows the persisted workspace fetch timestamp after reload", async ({
		page,
		gitbutler,
	}) => {
		await gitbutler.runScript("project-with-remote-branches.sh");
		await openWorkspace(page);

		const syncButton = getByTestId(page, "sync-button");
		await expect(syncButton).toContainText("Refetch");

		await clickByTestId(page, "sync-button");
		await expect(syncButton).toContainText(/Just now|A few sec ago/);

		await page.reload();
		await expect(getByTestId(page, "workspace-view")).toBeVisible();
		await expect(getByTestId(page, "sync-button")).toContainText(/Just now|A few sec ago/);
	});

	test("fetches healthy remotes even when another remote fails", async ({ page, gitbutler }) => {
		await gitbutler.runScript("project-with-remote-branches.sh");
		await openWorkspace(page);

		await gitbutler.runScript("project-with-remote-branches__add-commit-to-base.sh");

		const localClone = gitbutler.pathInWorkdir("local-clone");
		const remoteProject = gitbutler.pathInWorkdir("remote-project");
		const missingRemote = gitbutler.pathInWorkdir("missing-remote");
		git(localClone, ["remote", "add", "broken", missingRemote]);

		const expectedOriginMaster = git(remoteProject, ["rev-parse", "master"]);
		expect(git(localClone, ["rev-parse", "origin/master"])).not.toBe(expectedOriginMaster);

		const fetchResponse = page.waitForResponse(
			(response) =>
				response.url().endsWith("/workspace_fetch_from_remotes") &&
				response.request().method() === "POST",
		);
		await clickByTestId(page, "sync-button");

		// The target's remote (origin) is healthy, so the broken unrelated remote must not
		// fail the operation (a failure response is what triggers the error toast).
		expect(await (await fetchResponse).json()).toMatchObject({ type: "success" });

		await expect
			.poll(() => git(localClone, ["rev-parse", "origin/master"]), {
				message: "Expected origin/master to update even though another remote failed",
				intervals: [100, 200, 500, 1000],
			})
			.toBe(expectedOriginMaster);
	});
});
