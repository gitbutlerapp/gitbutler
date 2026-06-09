import { openWorkspace } from "../../src/setup.ts";
import { expect, type Page } from "@playwright/test";

export const SINGLE_BRANCH_NAME = "single-branch-fixture";

export async function openSingleBranchWorkspace(page: Page): Promise<void> {
	await openWorkspace(page);
	await expect(page.getByTestId("workspace-view")).toBeVisible();
}

export async function expectCurrentBranchChip(page: Page, branchName: string): Promise<void> {
	await expect(page.getByTestId("chrome-header-current-branch")).toContainText(branchName);
}
