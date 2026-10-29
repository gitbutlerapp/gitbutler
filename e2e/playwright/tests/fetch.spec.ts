import { expect } from "../src/expect.ts";
import { forgeErrorBody, githubForgeInfo, mockForge } from "../src/forge.ts";
import { openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, getByTestId } from "../src/util.ts";
import { type Page, type Request } from "@playwright/test";
import { execFileSync } from "node:child_process";

function git(pathToRepo: string, args: string[]): string {
	return execFileSync("git", args, {
		cwd: pathToRepo,
		encoding: "utf8",
	}).trim();
}

function backendSuccess(subject: unknown): string {
	return JSON.stringify({ type: "success", subject });
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

	test("repeated sync stays quiet when the forge is not authenticated", async ({
		page,
		gitbutler,
	}) => {
		await gitbutler.runScript("project-with-remote-branches.sh");
		await mockForge(page, { forge_info: githubForgeInfo() });

		// Model a disconnected integration: the live refresh fails with
		// `ForgeNotAuthenticated` while cached reads serve stale data.
		let liveReviewRequests = 0;
		await page.route("**/list_reviews", async (route) => {
			const isLive = route.request().postDataJSON()?.cacheConfig === "noCache";
			if (isLive) liveReviewRequests += 1;
			await route.fulfill({
				status: 200,
				contentType: "application/json",
				body: isLive
					? forgeErrorBody({
							code: "ForgeNotAuthenticated",
							message: "Not authenticated with GitHub.",
						})
					: backendSuccess([]),
			});
		});

		await openWorkspace(page);
		const syncButton = getByTestId(page, "sync-button");
		for (let attempt = 0; attempt < 2; attempt += 1) {
			const fetchStatusResponse = page.waitForResponse(
				(response) =>
					response.url().endsWith("/workspace_fetch_status") &&
					response.request().method() === "POST",
			);
			await clickByTestId(page, "sync-button");
			await fetchStatusResponse;
			await expect(syncButton).not.toContainText("Fetching...");
		}

		// The refresh is still attempted every sync; only the auth error is muted.
		expect(liveReviewRequests).toBe(2);
		// Match both the classified copy ("You are not logged in to your
		// forge...") and the raw backend message ("Not authenticated with
		// GitHub."), so the test still catches a regression where the error
		// slips through unclassified and toasts with the raw wording.
		await expect(
			page.getByTestId("toast-info-message").filter({ hasText: /not (logged in|authenticated)/i }),
		).toHaveCount(0);
	});

	test("sync surfaces review refresh failures that are not auth-related", async ({
		page,
		gitbutler,
	}) => {
		await gitbutler.runScript("project-with-remote-branches.sh");
		await mockForge(page, { forge_info: githubForgeInfo() });

		await page.route("**/list_reviews", async (route) => {
			const isLive = route.request().postDataJSON()?.cacheConfig === "noCache";
			await route.fulfill({
				status: 200,
				contentType: "application/json",
				body: isLive
					? forgeErrorBody({ code: "GitHubTokenExpired", message: "401: bad credentials" })
					: backendSuccess([]),
			});
		});

		await openWorkspace(page);
		const fetchStatusResponse = page.waitForResponse("**/workspace_fetch_status");
		await clickByTestId(page, "sync-button");
		await fetchStatusResponse;
		await expect(getByTestId(page, "sync-button")).not.toContainText("Fetching...");

		await expect(
			page.getByTestId("toast-info-message").filter({ hasText: "token appears expired" }),
		).toHaveCount(1);
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
