import type { App } from "../app.ts";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

const openSettings = (app: App) =>
	app.getByRole("button", { name: "Settings", exact: true }).click();
const checkButton = (app: App) =>
	app.getByRole("button", { name: "Check for updates", exact: true });
const closeSettings = async (app: App) => {
	await app.press("Escape");
	await expect(app.getByRole("dialog", { name: "Settings", exact: true })).toHaveCount(0);
};

const announce = (app: App, status: unknown) => app.backend.send("updateStatusChange", status);

const downloading = (percent: number, transferred: number) => ({
	_tag: "Downloading",
	version: "0.0.201",
	progress: { percent, transferred, total: 150_000_000, delta: 1_000_000, bytesPerSecond: 500_000 },
});

test("settings permits a manual check with automatic checks disabled", async ({ app }) => {
	await openSettings(app);
	await expect(
		app.getByRole("switch", { name: "Check for updates automatically" }),
	).not.toBeChecked();

	await checkButton(app).click();

	await expect(app.getByText("Updates are unavailable in this build.")).toBeVisible();
});

test("the unavailable notice goes away on its own", async ({ app }) => {
	await openSettings(app);
	await checkButton(app).click();
	await closeSettings(app);

	await expect(app.getByText("Updates are unavailable in this build.")).not.toBeVisible({
		timeout: 10_000,
	});
});

test("a manual check is busy until it has an answer", async ({ app }) => {
	const answer = await app.backend.answerLater("checkForUpdates");
	await openSettings(app);

	await checkButton(app).click();
	await expect(checkButton(app)).toHaveAttribute("aria-busy", "true");

	await answer({ _tag: "Available", version: "0.0.201" });
	await expect(checkButton(app)).toHaveAttribute("aria-busy", "false");
});

test("an available update offers its download in a nonmodal notice", async ({ app }) => {
	await app.backend.stub({ checkForUpdates: { _tag: "Available", version: "0.0.201" } });
	await openSettings(app);
	await checkButton(app).click();
	await closeSettings(app);

	await expect(app).toShow(
		"Update available. Download the update now and restart when you're ready.",
	);
	await expect(app.getByRole("button", { name: "Download 0.0.201" })).toBeVisible();
	const modal = app.page.locator('[role="dialog"][aria-modal="true"]');
	await expect(modal).toHaveCount(0);
});

test("a download reports its progress", async ({ app }) => {
	await announce(app, downloading(42, 63_000_000));

	await expect(app).toShow("Downloading update 0.0.201... 42% (63 MB / 150 MB)");
	await expect(app.getByRole("button", { name: "Download 0.0.201" })).toBeHidden();
});

test("a finished download prepares before it can install", async ({ app }) => {
	await announce(app, downloading(100, 150_000_000));

	await expect(app).toShow("Preparing update 0.0.201...");
	await expect(app.getByRole("button", { name: "Install 0.0.201 now" })).toBeHidden();
});

test("a ready update offers to install now or on quit", async ({ app }) => {
	await announce(app, { _tag: "Ready", version: "0.0.201" });

	await expect(app.getByRole("button", { name: "Install 0.0.201 now" })).toBeVisible();
	await expect(app).toShow("Update downloaded. Restart now or install on quit.");
});
