import { getButlerPort, type GitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, commitRow, fillByTestId, getByTestId, waitForTestId } from "../src/util.ts";
import { expect, type Page } from "@playwright/test";

test.describe.configure({ mode: "serial" });

/**
 * Bring up the hook-equipped project, enable hooks for the project, and reload
 * so the settings take effect. Returns once the workspace is visible.
 */
async function setupWithHooks(page: Page, gitbutler: GitButler) {
	await gitbutler.runScript("project-with-commit-hooks.sh");
	await page.goto("/");
	await waitForTestId(page, "workspace-view");

	const projectId = page.url().split("/")[3];
	await page.evaluate((id) => {
		localStorage.setItem(`projectRunCommitHooks_${id}`, "true");
	}, projectId);

	const response = await page.request.post(`http://localhost:${getButlerPort()}/update_project`, {
		data: { project: { id: projectId, husky_hooks_enabled: true } },
	});
	expect(response.ok()).toBeTruthy();
	expect((await response.json()).type).toBe("success");

	await page.reload();
	await waitForTestId(page, "workspace-view");
}

async function tryCommit(page: Page, title: string) {
	await clickByTestId(page, "start-commit-button");
	await expect(getByTestId(page, "new-commit-view")).toBeVisible();
	await fillByTestId(page, "commit-drawer-title-input", title);
	await clickByTestId(page, "commit-drawer-action-button");
}

test.describe("commit-msg hook", () => {
	test.use({ gitbutlerOptions: { env: { RUST_LOG: "debug" } } });

	test("rejection shows error toast", async ({ page, gitbutler }) => {
		await setupWithHooks(page, gitbutler);

		const fileList = getByTestId(page, "file-list-item");
		await expect(fileList).toHaveCount(1);
		await expect(fileList).toContainText("uncommitted.txt");

		await tryCommit(page, "This message should REJECT");

		const toast = getByTestId(page, "toast-info-message");
		await expect(toast).toBeVisible();
		await expect(toast).toContainText("REJECT");
	});
});

test("shows modified commit message from commit-msg hook", async ({ page, gitbutler }) => {
	await setupWithHooks(page, gitbutler);

	await expect(getByTestId(page, "file-list-item")).toHaveCount(1);
	await tryCommit(page, "This message should MODIFY");

	const row = commitRow(page).first();
	await expect(row).toBeVisible();
	await expect(row).toContainText("[MODIFIED]");
});

test("allows commits when commit-msg hook passes", async ({ page, gitbutler }) => {
	await setupWithHooks(page, gitbutler);

	await expect(getByTestId(page, "file-list-item")).toHaveCount(1);
	await tryCommit(page, "A normal commit message");

	const row = commitRow(page).first();
	await expect(row).toBeVisible();
	await expect(row).toContainText("A normal commit message");
});

test("rejects commit when pre-commit hook fails", async ({ page, gitbutler }) => {
	await setupWithHooks(page, gitbutler);

	await gitbutler.runScript("create-forbidden-file.sh");
	await page.reload();
	await waitForTestId(page, "workspace-view");

	await expect(getByTestId(page, "file-list-item")).toHaveCount(2);

	await tryCommit(page, "Adding forbidden file");

	const toast = getByTestId(page, "toast-info-message");
	await expect(toast).toBeVisible();
	await expect(toast).toContainText("FORBIDDEN");
});

test("allows commit when pre-commit hook passes", async ({ page, gitbutler }) => {
	await setupWithHooks(page, gitbutler);

	await gitbutler.runScript("create-allowed-file.sh");
	await page.reload();
	await waitForTestId(page, "workspace-view");

	await tryCommit(page, "Adding allowed file");

	const row = commitRow(page).first();
	await expect(row).toBeVisible();
	await expect(row).toContainText("Adding allowed file");
});

test("post-commit hook success", async ({ page, gitbutler }) => {
	await setupWithHooks(page, gitbutler);

	await expect(getByTestId(page, "file-list-item").first()).toBeVisible();
	await tryCommit(page, "Testing post-commit hook");

	const row = commitRow(page).first();
	await expect(row).toBeVisible();
	await expect(row).toContainText("Testing post-commit hook");

	// Post-commit hook runs in the background.
	await page.waitForTimeout(1500);
});

test("post-commit hook failure does not block commit", async ({ page, gitbutler }) => {
	await setupWithHooks(page, gitbutler);

	await gitbutler.runScript("create-postcommit-fail-marker.sh");
	await page.reload();
	await waitForTestId(page, "workspace-view");

	await tryCommit(page, "Trigger post-commit failure");

	const row = commitRow(page).first();
	await expect(row).toBeVisible();
	await expect(row).toContainText("Trigger post-commit failure");

	await page.waitForTimeout(1500);
});
