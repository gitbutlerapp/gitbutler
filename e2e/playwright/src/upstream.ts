import { clickByTestId, getByTestId } from "./util.ts";
import { expect, type Page } from "@playwright/test";

export async function fetchRemoteChanges(page: Page): Promise<void> {
	await clickByTestId(page, "sync-button");
}

export async function integrateUpstreamChanges(page: Page): Promise<void> {
	await fetchRemoteChanges(page);
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");
}

export async function pushFirstStack(page: Page): Promise<void> {
	const pushButton = getByTestId(page, "stack-push-button").first();
	await expect(pushButton).toBeEnabled();
	await pushButton.click();

	const confirmButton = getByTestId(page, "stack-confirm-push-modal-button");
	if (await confirmButton.isVisible({ timeout: 1000 })) {
		await confirmButton.click();
	}
}
