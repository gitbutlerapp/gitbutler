// The update-from-remote flow, applied for real with every answer the
// dialog offers — Keep mine, Combine both, Take theirs — against the
// divergences `diverge.ts` builds, asserting the branch history git actually
// ends up with.

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
	await expect(appWindow.getByText("origin/branch1")).toBeVisible();
	// A rewritten-only leg keeps its button behind the chevron.
	const integrate = appWindow.getByLabel("Integrate origin/branch1 into branch1");
	if ((await integrate.count()) === 0) await appWindow.getByLabel("Expand origin/branch1").click();
	await integrate.click();
	// The summary renders once the dry-run preview for the default answer is in.
	await expect(appWindow.getByText(/incoming commit/)).toBeVisible();
};

/** Pick one of the three answers and run its action. */
const applyChoice = async (
	appWindow: Page,
	choice: "Keep mine" | "Combine both" | "Take theirs",
	action: "Force push" | "Integrate" | "Replace branch",
): Promise<void> => {
	const chip = appWindow.locator(dialog).getByRole("button", { name: choice, exact: true });
	// The default choice's chip is already pressed, and a pressed toggle does
	// not take another click.
	if ((await chip.getAttribute("aria-pressed")) !== "true") await chip.click();
	const button = appWindow.locator(dialog).getByRole("button", { name: action, exact: true });
	// Disabled while the outline still shows the previous choice's data.
	await expect(button).toBeEnabled();
	await button.click();
	// Every answer ends in a done state that says what happened.
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

/** Leave the done state — after publishing from it, or not. */
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

/** After a successful update nothing is left to integrate, so the leg goes. */
const expectLegGone = async (appWindow: Page): Promise<void> => {
	await expect(appWindow.getByText("origin/branch1")).toBeHidden();
};

test.describe("update from remote", () => {
	// Each test launches Electron, seeds a repository, builds the divergence,
	// and runs a real integration; the 30s default is not enough.
	test.describe.configure({ timeout: 180_000 });
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("a rewritten branch's leg lists the remote's version of its commits", async ({
		appWindow,
		testEnvironment,
	}) => {
		rewriteBranch1Tip(testEnvironment);
		await appWindow.reload();
		await appWindow.getByRole("main").waitFor();

		// Head info prunes the remote twin, so the summary can only name the
		// state — quietly, since a rewrite only ever wants a force push — and
		// the expanded leg fetches the remote's versions instead.
		await expect(appWindow.getByText("origin/branch1")).toBeVisible();
		await expect(appWindow.getByText("rewritten", { exact: true })).toBeVisible();
		await expect(appWindow.getByLabel("Integrate origin/branch1 into branch1")).toHaveCount(0);
		await appWindow.getByLabel("Expand origin/branch1").click();
		// The remote still holds the pre-reword subject, which the local side
		// no longer shows anywhere.
		await expect(appWindow.getByText("branch1: second commit")).toBeVisible();
	});

	test("keep mine force-pushes the rewritten branch", async ({ appWindow, testEnvironment }) => {
		rewriteBranch1Tip(testEnvironment);
		await openUpdateDialog(appWindow);

		// The rewrite-only diagnosis, and Keep mine as the standing choice:
		// with nothing to take, the action is the push itself.
		await expect(
			appWindow.getByText("origin/branch1 holds a different version of one of your commits."),
		).toBeVisible();
		await applyChoice(appWindow, "Keep mine", "Force push");
		await finishWith(appWindow, "Done");

		// The remote now holds exactly the local branch, and the leg is gone.
		const clone = path.join(testEnvironment.workdir, "local-clone");
		const local = execFileSync("git", ["-C", clone, "rev-parse", "refs/heads/branch1"], {
			encoding: "utf8",
		}).trim();
		const remote = execFileSync("git", ["-C", clone, "ls-remote", "origin", "refs/heads/branch1"], {
			encoding: "utf8",
		}).split("\t")[0];
		expect(remote).toBe(local);
		await expectLegGone(appWindow);
	});

	test("combine both puts their new commits under your work", async ({
		appWindow,
		testEnvironment,
	}) => {
		divergeBranch1(testEnvironment);
		await openUpdateDialog(appWindow);

		// The diagnosis states what the remote has; with only additions there
		// is nothing to choose, and the one action integrates. The local
		// commit and the remote rework both edit a_file, and the preview says
		// so before anything is applied.
		await expect(appWindow.getByText("origin/branch1 has 2 new commits.")).toBeVisible();
		await expect(appWindow.getByText("1 commit will need conflict resolution.")).toBeVisible();
		// The result sits on the remote's tip, so a plain push publishes it —
		// said before applying, and offered once applied.
		await expect(appWindow.getByText("Push afterwards to publish.")).toBeVisible();
		await applyChoice(appWindow, "Combine both", "Integrate");
		await expect(appWindow.locator(dialog).getByRole("button", { name: "Push" })).toBeVisible();
		await finishWith(appWindow, "Done");
		await expectLegGone(appWindow);

		// Your commit, rebased onto their rework, conflicts — and its subject says so.
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

		// Both kinds at once: a new commit to take, and an older version of
		// the reworded commit that must be left out rather than picked below
		// the version that superseded it.
		await expect(
			appWindow.getByText(
				"origin/branch1 has 1 new commit, and a different version of one of yours.",
			),
		).toBeVisible();
		await expect(
			appWindow.getByText("Rewrites the branch; force push afterwards to publish."),
		).toBeVisible();
		await applyChoice(appWindow, "Combine both", "Integrate");
		// The remote still holds the superseded version, so the done state
		// offers the force push right there; taking it publishes the result.
		await finishWith(appWindow, "Force push");
		await expectLegGone(appWindow);

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
		// The remote's version of the reworded commit is nowhere in the result.
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
		await expectLegGone(appWindow);

		// Nothing integrated: their new commits are gone from the remote too.
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
		await expectLegGone(appWindow);

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
