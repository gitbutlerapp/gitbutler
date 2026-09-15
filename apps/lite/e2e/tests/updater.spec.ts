import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

test("settings permits a manual check with automatic checks disabled", async ({
	appWindow,
}, testInfo) => {
	await appWindow.getByRole("button", { name: "Settings", exact: true }).click();
	await expect(
		appWindow.getByRole("switch", { name: "Check for updates automatically" }),
	).not.toBeChecked();
	const check = appWindow.getByRole("button", { name: "Check for updates", exact: true });
	await expect(check).toBeVisible();
	const screenshot = testInfo.outputPath("settings-updates.png");
	await appWindow.screenshot({ path: screenshot });
	await testInfo.attach("settings-updates", { path: screenshot, contentType: "image/png" });
	await check.click();
	await expect(appWindow.getByText("Updates are unavailable in this build.")).toBeVisible();
	await appWindow.keyboard.press("Escape");
	await expect(appWindow.getByRole("dialog", { name: "Settings", exact: true })).toHaveCount(0);
	await expect(appWindow.getByText("Updates are unavailable in this build.")).not.toBeVisible({
		timeout: 10000,
	});
});

test("check results and native preparation render nonmodal notices with explicit actions", async ({
	appWindow,
	electronApp,
}, testInfo) => {
	const finishCheck = await electronApp.evaluateHandle(({ ipcMain }) => {
		const { promise, resolve } = Promise.withResolvers<{ _tag: "Available"; version: string }>();
		ipcMain.removeHandler("checkForUpdates");
		ipcMain.handle("checkForUpdates", () => promise);
		return () => resolve({ _tag: "Available", version: "0.0.201" });
	});
	await appWindow.getByRole("button", { name: "Settings", exact: true }).click();
	const check = appWindow.getByRole("button", { name: "Check for updates", exact: true });
	await check.click();
	const spinner = check.locator('[data-icon][class*="spinning"]');
	await expect(check).toBeDisabled();
	await expect(spinner).toBeVisible();
	await finishCheck.evaluate((finish) => finish());
	await finishCheck.dispose();
	await expect(check).toBeEnabled();
	await expect(spinner).toHaveCount(0);
	await appWindow.keyboard.press("Escape");
	// The button shares the toast's title, so wait until the dialog has taken it away.
	await expect(appWindow.getByRole("dialog", { name: "Settings", exact: true })).toHaveCount(0);
	await expect(appWindow.getByText("Check for updates", { exact: true })).toBeVisible();
	await expect(
		appWindow.getByText(
			"Update available. Download the update now and restart when you're ready.",
			{ exact: true },
		),
	).toBeVisible();
	await expect(appWindow.getByRole("button", { name: "Download 0.0.201" })).toBeVisible();
	await expect(appWindow.locator('[role="dialog"][aria-modal="true"]')).toHaveCount(0);
	const screenshot = testInfo.outputPath("update-available.png");
	await appWindow.screenshot({ path: screenshot });
	await testInfo.attach("update-available", {
		path: screenshot,
		contentType: "image/png",
	});
	await electronApp.evaluate(({ BrowserWindow }) => {
		BrowserWindow.getAllWindows()[0]?.webContents.send("updateStatusChange", {
			_tag: "Downloading",
			version: "0.0.201",
			progress: {
				percent: 42,
				transferred: 63_000_000,
				total: 150_000_000,
				delta: 1_000_000,
				bytesPerSecond: 500_000,
			},
		});
	});
	await expect(
		appWindow.getByText("Downloading update 0.0.201... 42% (63 MB / 150 MB)", { exact: true }),
	).toBeVisible();
	await expect(appWindow.getByRole("button", { name: "Download 0.0.201" })).not.toBeVisible();
	await electronApp.evaluate(({ BrowserWindow }) => {
		BrowserWindow.getAllWindows()[0]?.webContents.send("updateStatusChange", {
			_tag: "Downloading",
			version: "0.0.201",
			progress: {
				percent: 100,
				transferred: 150_000_000,
				total: 150_000_000,
				delta: 87_000_000,
				bytesPerSecond: 500_000,
			},
		});
	});
	await expect(appWindow.getByText("Preparing update 0.0.201...", { exact: true })).toBeVisible();
	await expect(appWindow.getByRole("button", { name: "Install 0.0.201 now" })).not.toBeVisible();
	await electronApp.evaluate(({ BrowserWindow }) => {
		BrowserWindow.getAllWindows()[0]?.webContents.send("updateStatusChange", {
			_tag: "Ready",
			version: "0.0.201",
		});
	});
	await expect(appWindow.getByRole("button", { name: "Install 0.0.201 now" })).toBeVisible();
	await expect(
		appWindow.getByText("Update downloaded. Restart now or install on quit.", { exact: true }),
	).toBeVisible();
	await expect(appWindow.getByRole("button", { name: "Download 0.0.201" })).not.toBeVisible();
});
