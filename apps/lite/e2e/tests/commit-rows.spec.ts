import type { App } from "../app.ts";
import { expect, test } from "../test.ts";

const commitMenu = { name: "Commit menu", exact: true } as const;

test.describe("workspace commit rows", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	test("are a single line", async ({ app }) => {
		await expect(app.commit("B: first commit")).toHaveHeight(28);
	});

	test("keep their label in place on hover", async ({ app }) => {
		const row = app.commit("B: first commit");
		const label = row.getByText("B: first commit", { exact: true }).locator("..");

		await expect(label).toHoldStillOnHover({ revealing: row.getByRole("button", commitMenu) });
	});

	test("Escape leaves an inline message edit unsaved", async ({ app }) => {
		const editor = await app.editCommitMessage("B: first commit");
		await editor.fill("An unsaved message");

		await editor.press("Escape");

		await expect(app.commit("B: first commit")).toBeVisible();
	});
});

test.describe("unapplied commit rows", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test.beforeEach(async ({ app }) => {
		await app.goTo("Branches");
		await app.branch("branch1").getByRole("button", { name: "Unfold commits" }).click();
	});

	test("are a single line", async ({ app }) => {
		await expect(app.commit("branch1: first commit")).toHaveHeight(28);
	});

	test("keep the menu out of the label's space", async ({ app }) => {
		const row = app.commit("branch1: first commit");
		const label = row.getByText("branch1: first commit", { exact: true }).locator("..");

		await expect(label).toHoldStillOnHover({ revealing: row.getByRole("button", commitMenu) });
	});

	test("select on click", async ({ app }) => {
		await app.commit("branch1: first commit").click();

		await expect(app.commit("branch1: first commit")).toBeSelected();
	});
});

const unfold = { upstream: "Unfold incoming commits", history: "Unfold history" };

test.describe("target commit rows", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	for (const section of ["upstream", "history"] as const) {
		test(`show PR labels unclipped in ${section}`, async ({ app }) => {
			await showTargetCommitWithReview(app, section);

			await app.getByRole("button", { name: unfold[section], exact: true }).click();

			const row = app.commit("Improve parser diagnostics");
			await expect(row.getByText("@gitbutler/lite", { exact: true })).toShowItsTextUnclipped();
			await expect(row).toHaveAccessibleDescription(/@gitbutler\/lite/);
		});
	}
});

/** The newest target commit, as the forge would describe it in `section`, carrying a labelled PR. */
const showTargetCommitWithReview = async (app: App, section: "upstream" | "history") => {
	const listing = await app.page.evaluate(
		(projectId) => window.lite.workspaceTargetCommits({ projectId, from: null, limit: null }),
		app.projectId,
	);
	const target = listing.commits[0];
	if (target === undefined) throw new Error("No target commit");
	const withReview = {
		...listing,
		commits: [
			{
				...target,
				inWorkspace: section === "history",
				commit: {
					...target.commit,
					id: section === "upstream" ? "a".repeat(40) : target.commit.id,
					committedAt: Date.now(),
				},
				review: {
					number: 42,
					title: "Improve parser diagnostics",
					htmlUrl: "https://example.com/pull/42",
					unitSymbol: "#",
					sourceBranch: "parser-diagnostics",
					labels: [{ name: "@gitbutler/lite", color: "5319e7", description: "Lite changes" }],
				},
			},
			...listing.commits.slice(section === "upstream" ? 0 : 1),
		],
	};
	await app.electronApp.evaluate(({ ipcMain }, listing) => {
		ipcMain.removeHandler("workspaceTargetCommits");
		ipcMain.handle("workspaceTargetCommits", (_event, { from }: { from: string | null }) =>
			from === null ? listing : { commits: [], hasMore: false },
		);
	}, withReview);
	await app.reload();
};
