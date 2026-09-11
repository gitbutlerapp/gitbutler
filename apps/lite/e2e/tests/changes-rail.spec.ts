import type { TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import type { LiteElectronApi } from "#electron/ipc.ts";
import { enabled, openProject, shoot } from "../screenshot-helpers.ts";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

test("shows change volume in flat and tree lists and folds reviewed folders", async ({
	appWindow,
	electronApp,
}) => {
	await openProject(appWindow);
	const sample = await appWindow.evaluate(async () => {
		const projectId = location.pathname.split("/")[2] ?? "";
		return (window as unknown as { lite: LiteElectronApi }).lite.branchDiff({
			projectId,
			branch: "refs/heads/C",
		});
	});
	const first = sample.changes[0];
	if (first?.status.type !== "Addition") throw new Error("expected seeded added file");
	const state = first.status.subject.state;
	const changes: Array<TreeChange> = Array.from({ length: 34 }, (_, index) => {
		const name =
			index === 0
				? "Graph/usePlan.ts"
				: index === 1
					? "src/large.ts"
					: `src/group-${index % 4}/file-${index}.ts`;
		return {
			path: name,
			pathBytes: Array.from(Buffer.from(name)),
			status:
				index < 30
					? { type: "Modification", subject: { previousState: state, state, flags: null } }
					: index < 33
						? { type: "Addition", subject: { state, isUntracked: false } }
						: { type: "Deletion", subject: { previousState: state } },
		};
	});
	const patches: Record<string, UnifiedPatch> = Object.fromEntries<UnifiedPatch>(
		changes.map((change, index): [string, UnifiedPatch] => {
			if (index === 2) return [change.path, { type: "Binary" }];
			const added = index === 1 ? 120 : index === 33 ? 0 : 1;
			const removed = index < 30 || index === 33 ? 1 : 0;
			return [
				change.path,
				{
					type: "Patch",
					subject: {
						isResultOfBinaryToTextConversion: false,
						linesAdded: added,
						linesRemoved: removed,
						hunks: [
							{
								oldStart: removed > 0 ? 1 : 0,
								oldLines: removed,
								newStart: added > 0 ? 1 : 0,
								newLines: added,
								diff: `@@ -${removed > 0 ? 1 : 0},${removed} +${added > 0 ? 1 : 0},${added} @@\n${removed > 0 ? "-before\n" : ""}${"+after\n".repeat(added)}`,
							},
						],
					},
				},
			];
		}),
	);
	await electronApp.evaluate(
		({ ipcMain }, { sample, changes, patches }) => {
			ipcMain.removeHandler("branchDiff");
			ipcMain.handle("branchDiff", () => ({ ...sample, changes }));
			ipcMain.removeHandler("treeChangeDiffs");
			ipcMain.handle(
				"treeChangeDiffs",
				(_event, { change }: { change: TreeChange }) => patches[change.path],
			);
		},
		{ sample, changes, patches },
	);
	await appWindow.reload();
	const details = appWindow.locator("#details-panel");
	await expect(details.getByText("34 files", { exact: true })).toBeVisible();
	await details.getByRole("button", { name: "Flat", exact: true }).click();
	await details
		.getByRole("tree")
		.evaluate((tree) => tree.parentElement?.scrollTo({ top: tree.parentElement.scrollHeight }));
	const largest = details.getByRole("treeitem", { name: "Modification src/large.ts", exact: true });
	await expect(largest.getByText("+120", { exact: true })).toBeVisible();
	await expect(largest.getByLabel("Modification", { exact: true })).toHaveCount(0);
	await details.getByRole("button", { name: "Tree", exact: true }).click();
	await details.getByRole("tree").evaluate((tree) => tree.parentElement?.scrollTo({ top: 0 }));
	await expect(details.getByText("Graph/usePlan.ts", { exact: true })).toBeVisible();
	const folder = details.getByRole("treeitem", { name: "Directory src", exact: true });
	await expect(folder.getByText("33", { exact: true })).toBeVisible();
	await expect(folder.getByText("+150", { exact: true })).toBeVisible();
	await details.getByRole("button", { name: "Mark all reviewed", exact: true }).click();
	await expect(folder).toHaveAttribute("aria-expanded", "false");
	if (enabled) await shoot(appWindow, "changes-rail", "#details-panel");
	await folder.getByRole("button", { name: "Expand directory src", exact: true }).click();
	await expect(folder).toHaveAttribute("aria-expanded", "true");
	await appWindow.reload();
	await expect(details.getByRole("button", { name: "Tree", exact: true })).toHaveAttribute(
		"aria-pressed",
		"true",
	);
	await expect(folder).toHaveAttribute("aria-expanded", "false");
});
