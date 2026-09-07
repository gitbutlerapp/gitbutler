// The update-from-remote flow applied for real with every answer, against the
// divergences diverge.ts builds, asserting the resulting git history.

import { execFileSync } from "node:child_process";
import path from "node:path";
import type { Page } from "@playwright/test";
import { divergeBoth, divergeBranch1, rewriteBranch1Tip } from "../diverge.ts";
import { expect, test } from "../test.ts";

const dialog = '[aria-labelledby="branch-update-heading"]';

const logSubjects = (clone: string): Array<string> =>
	execFileSync("git", ["-C", clone, "log", "--format=%s", "refs/heads/branch1"], {
		encoding: "utf8",
	})
		.trim()
		.split("\n");

const openUpdateDialog = async (appWindow: Page): Promise<void> => {
	await appWindow.reload();
	await appWindow.getByRole("main").waitFor();
	await appWindow.getByLabel("Integrate origin/branch1 into branch1").click();
	// The summary renders once the dry-run preview for the default answer is in.
	await expect(appWindow.getByText(/incoming commit/)).toBeVisible();
};

const applyChoice = async (
	appWindow: Page,
	choice: "Keep mine" | "Combine both" | "Take theirs",
	action: "Force push" | "Integrate" | "Replace branch",
): Promise<void> => {
	const chip = appWindow.locator(dialog).getByRole("button", { name: choice, exact: true });
	// A pressed toggle does not take another click.
	if ((await chip.getAttribute("aria-pressed")) !== "true") await chip.click();
	const button = appWindow.locator(dialog).getByRole("button", { name: action, exact: true });
	// Disabled while the outline still shows the previous choice's data.
	await expect(button).toBeEnabled();
	await button.click();
	await expect(
		appWindow.getByText(
			action === "Integrate"
				? "Branch updated."
				: action === "Force push"
					? "Branch published."
					: "Branch replaced.",
		),
	).toBeVisible();
};

const finishWith = async (
	appWindow: Page,
	action: "Done" | "Push" | "Force push",
): Promise<void> => {
	if (action !== "Done") {
		await appWindow.locator(dialog).getByRole("button", { name: action, exact: true }).click();
		await expect(appWindow.getByText("Branch published.")).toBeVisible();
	}
	await appWindow.locator(dialog).getByRole("button", { name: "Done", exact: true }).click();
	await expect(appWindow.locator(dialog)).toBeHidden();
};

const expectIncomingGone = async (appWindow: Page): Promise<void> => {
	await expect(appWindow.getByRole("button", { name: /incoming commit/ })).toHaveCount(0);
};

test.describe("update from remote", () => {
	// Each test launches Electron and runs a real integration; 30s is not enough.
	test.describe.configure({ timeout: 180_000 });
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("a rewritten branch says so in its row and has nothing to bring in", async ({
		appWindow,
		testEnvironment,
	}) => {
		rewriteBranch1Tip(testEnvironment);
		await appWindow.reload();
		await appWindow.getByRole("main").waitFor();

		// Head info prunes the remote twin, so nothing is new: the word carries the state.
		await expect(appWindow.getByText("Rewritten", { exact: true })).toBeVisible();
		await expect(appWindow.getByRole("button", { name: /incoming commit/ })).toHaveCount(0);
		await expect(appWindow.getByLabel("Integrate origin/branch1 into branch1")).toHaveCount(0);
	});

	test("keep mine force-pushes the rewritten branch over their additions", async ({
		appWindow,
		testEnvironment,
	}) => {
		divergeBoth(testEnvironment);
		await openUpdateDialog(appWindow);

		// Keep mine with a twin and an addition to drop: the action is the push itself.
		await applyChoice(appWindow, "Keep mine", "Force push");
		await finishWith(appWindow, "Done");

		const clone = path.join(testEnvironment.workdir, "local-clone");
		const local = execFileSync("git", ["-C", clone, "rev-parse", "refs/heads/branch1"], {
			encoding: "utf8",
		}).trim();
		const remote = execFileSync("git", ["-C", clone, "ls-remote", "origin", "refs/heads/branch1"], {
			encoding: "utf8",
		}).split("\t")[0];
		expect(remote).toBe(local);
		await expectIncomingGone(appWindow);
	});

	test("combine both puts their new commits under your work", async ({
		appWindow,
		testEnvironment,
	}) => {
		divergeBranch1(testEnvironment);
		await openUpdateDialog(appWindow);
		await appWindow.keyboard.press("Escape");
		await expect(appWindow.locator(dialog)).toBeHidden();

		const chip = appWindow.getByRole("button", {
			name: "Show 2 incoming commits from origin/branch1",
		});
		await chip.click();
		await expect(appWindow.getByText("Rework the parser entry point")).toBeVisible();
		await expect(appWindow.getByText("Document the reworked entry point")).toBeVisible();
		await appWindow
			.getByRole("button", { name: "Hide 2 incoming commits from origin/branch1" })
			.click();

		await appWindow.getByLabel("Integrate origin/branch1 into branch1").click();
		await expect(appWindow.getByText(/incoming commit/)).toBeVisible();

		// Only additions, so Combine is preselected; the preview names the a_file conflict.
		await expect(appWindow.getByText("origin/branch1 has 2 new commits.")).toBeVisible();
		await expect(appWindow.getByText("1 commit will need conflict resolution.")).toBeVisible();
		// The result sits on the remote's tip: a plain push, said before and offered after.
		await expect(appWindow.getByText("Push afterwards to publish.")).toBeVisible();
		await applyChoice(appWindow, "Combine both", "Integrate");
		await expect(appWindow.locator(dialog).getByRole("button", { name: "Push" })).toBeVisible();
		await finishWith(appWindow, "Done");
		await expectIncomingGone(appWindow);

		// Your commit, rebased onto their rework, conflicts, and its subject says so.
		const clone = path.join(testEnvironment.workdir, "local-clone");
		expect(logSubjects(clone).slice(0, 4)).toEqual([
			"[conflict] branch1: second commit",
			"Document the reworked entry point",
			"Rework the parser entry point",
			"branch1: first commit",
		]);
	});

	test("unticking an incoming commit leaves it out of the integration", async ({
		appWindow,
		testEnvironment,
	}) => {
		divergeBranch1(testEnvironment);
		await openUpdateDialog(appWindow);

		// Pick: their additions each carry a checkbox, ticked by default.
		await appWindow.getByLabel("Keep Document the reworked entry point").click();
		await expect(appWindow.getByText("1 of 2 incoming commits")).toBeVisible();
		// The remote keeps the commit left out, so publishing means a force push.
		await expect(
			appWindow.getByText("Rewrites the branch; force push afterwards to publish."),
		).toBeVisible();
		await applyChoice(appWindow, "Combine both", "Integrate");
		await finishWith(appWindow, "Done");

		const clone = path.join(testEnvironment.workdir, "local-clone");
		const subjects = logSubjects(clone);
		expect(subjects).toContain("Rework the parser entry point");
		expect(subjects).not.toContain("Document the reworked entry point");
		expect(subjects[0]).toBe("[conflict] branch1: second commit");
	});

	test("combine both never lands a rewritten commit twice", async ({
		appWindow,
		testEnvironment,
	}) => {
		divergeBoth(testEnvironment);
		await openUpdateDialog(appWindow);

		// Both kinds: a new commit to take, and an older version of the reworded one to leave out.
		await expect(
			appWindow.getByText(
				"origin/branch1 has 1 new commit, and a different version of one of yours.",
			),
		).toBeVisible();
		await expect(
			appWindow.getByText("Rewrites the branch; force push afterwards to publish."),
		).toBeVisible();
		await applyChoice(appWindow, "Combine both", "Integrate");
		// The remote still holds the superseded version, so the done state offers the force push.
		await finishWith(appWindow, "Force push");
		await expectIncomingGone(appWindow);

		const clone = path.join(testEnvironment.workdir, "local-clone");
		const subjects = logSubjects(clone);
		const remote = execFileSync("git", ["-C", clone, "ls-remote", "origin", "refs/heads/branch1"], {
			encoding: "utf8",
		}).split("\t")[0];
		const local = execFileSync("git", ["-C", clone, "rev-parse", "refs/heads/branch1"], {
			encoding: "utf8",
		}).trim();
		expect(remote).toBe(local);
		expect(subjects.slice(0, 3)).toEqual([
			"Reworded locally",
			"Add upstream notes",
			"branch1: first commit",
		]);
		expect(subjects).not.toContain("branch1: second commit");
		expect(subjects.filter((subject) => subject.endsWith("Reworded locally"))).toHaveLength(1);
	});

	test("keep mine with additions force-pushes the branch as it is", async ({
		appWindow,
		testEnvironment,
	}) => {
		divergeBranch1(testEnvironment);
		await openUpdateDialog(appWindow);
		await applyChoice(appWindow, "Keep mine", "Force push");
		await finishWith(appWindow, "Done");
		await expectIncomingGone(appWindow);

		const clone = path.join(testEnvironment.workdir, "local-clone");
		const subjects = logSubjects(clone);
		expect(subjects.slice(0, 2)).toEqual(["branch1: second commit", "branch1: first commit"]);
		const remote = execFileSync("git", ["-C", clone, "ls-remote", "origin", "refs/heads/branch1"], {
			encoding: "utf8",
		}).split("\t")[0];
		const local = execFileSync("git", ["-C", clone, "rev-parse", "refs/heads/branch1"], {
			encoding: "utf8",
		}).trim();
		expect(remote).toBe(local);
	});

	test("take theirs replaces the branch with the remote", async ({
		appWindow,
		testEnvironment,
	}) => {
		divergeBranch1(testEnvironment);
		await openUpdateDialog(appWindow);
		await applyChoice(appWindow, "Take theirs", "Replace branch");
		await finishWith(appWindow, "Done");
		await expectIncomingGone(appWindow);

		const clone = path.join(testEnvironment.workdir, "local-clone");
		const subjects = logSubjects(clone);
		expect(subjects.slice(0, 3)).toEqual([
			"Document the reworked entry point",
			"Rework the parser entry point",
			"branch1: first commit",
		]);
		expect(subjects).not.toContain("branch1: second commit");
	});
});
