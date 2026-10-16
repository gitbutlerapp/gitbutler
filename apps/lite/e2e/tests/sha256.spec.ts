import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import path from "node:path";
import { expect, test } from "../test.ts";
import type { LiteTestEnvironment } from "../setup.ts";

const cloneOf = (environment: LiteTestEnvironment): string =>
	path.join(environment.workdir, "local-clone");

const git = (environment: LiteTestEnvironment, ...args: Array<string>): string =>
	execFileSync("git", ["-C", cloneOf(environment), ...args], { encoding: "utf8" }).trim();

const commitIdWithSubject = (environment: LiteTestEnvironment, subject: string): string =>
	git(environment, "log", "--all", "--format=%H", "--fixed-strings", `--grep=${subject}`);

test.describe("sha256 repository", () => {
	test.use({
		objectFormat: "sha256",
		scenario: "project-in-single-branch-three-branch-stack.sh",
	});

	test("lists the stack", async ({ appWindow, testEnvironment }) => {
		expect(git(testEnvironment, "rev-parse", "--show-object-format")).toBe("sha256");

		for (const branch of ["A", "B", "C"]) {
			await expect(
				appWindow.getByRole("treeitem", { name: `${branch}: first commit`, exact: true }),
			).toBeVisible();
		}
	});

	test("commits an uncommitted file", async ({ appWindow, testEnvironment }) => {
		writeFileSync(path.join(cloneOf(testEnvironment), "added.txt"), "an uncommitted file\n");
		await appWindow.reload();
		await appWindow.getByRole("main").waitFor();

		await appWindow
			.getByRole("tree", { name: "Uncommitted" })
			.getByRole("checkbox", { name: "Check file added.txt" })
			.click();
		await appWindow.getByRole("button", { name: /start commit/i }).click();
		await appWindow
			.getByRole("textbox", { name: "Compose commit message" })
			.fill("Add a file on sha256");
		await appWindow.getByRole("button", { name: "Commit", exact: true }).click();

		await expect(
			appWindow.getByRole("treeitem", { name: "Add a file on sha256", exact: true }),
		).toBeVisible();
		expect(commitIdWithSubject(testEnvironment, "Add a file on sha256")).toMatch(/^[0-9a-f]{64}$/);
	});

	test("rewords a commit below the tip", async ({ appWindow, testEnvironment }) => {
		const row = appWindow.getByRole("treeitem", { name: "A: first commit", exact: true });
		await row.getByText("A: first commit", { exact: true }).dblclick();
		const editor = row.getByRole("textbox", { name: "Commit message" });
		await editor.fill("A: reworded on sha256");
		await editor.press("Enter");

		await expect(
			appWindow.getByRole("treeitem", { name: "A: reworded on sha256", exact: true }),
		).toBeVisible();
		await expect(
			appWindow.getByRole("treeitem", { name: "C: first commit", exact: true }),
		).toBeVisible();
		expect(commitIdWithSubject(testEnvironment, "A: reworded on sha256")).toMatch(/^[0-9a-f]{64}$/);
	});
});
