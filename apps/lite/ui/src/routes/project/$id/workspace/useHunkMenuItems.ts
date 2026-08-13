import {
	useCommitDiscardChanges,
	useCommitUncommitChanges,
	useDiscardWorktreeChanges,
	useOpenInProgram,
} from "#ui/api/mutations.ts";
import {
	guiSettingsQueryOptions,
	listEditorsQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { diffHotkeys, selectionOperationHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import { diffSpecHunkHeadersForLineSelection } from "#ui/hunk.ts";
import { type NativeMenuItem, nativeMenuItem, nativeMenuItemsFromGroups } from "#ui/native-menu.ts";
import { hunkOperand, type HunkOperand } from "#ui/operands.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppStore } from "#ui/store.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { Match } from "effect";

type HunkMenuTarget = {
	change: TreeChange;
	lineNumber: number;
	operand: HunkOperand;
};

export const useHunkMenuItems = ({
	projectId,
}: {
	projectId: string;
}): ((target: HunkMenuTarget) => Array<NativeMenuItem>) => {
	const dispatch = useAppDispatch();
	const store = useAppStore();
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	const { isPending: isCommitUncommitChangesPending, mutate: commitUncommitChanges } =
		useCommitUncommitChanges();
	const { isPending: isCommitDiscardChangesPending, mutate: commitDiscardChanges } =
		useCommitDiscardChanges();
	const { isPending: isDiscardWorktreeChangesPending, mutate: discardWorktreeChanges } =
		useDiscardWorktreeChanges();
	const { isPending: isOpenInProgramPending, mutate: openInProgram } = useOpenInProgram();

	return ({ operand, change, lineNumber }) => {
		const source = hunkOperand(operand);
		const canUseHunk = !operand.isResultOfBinaryToTextConversion;
		const cutHunk = () => {
			const state = store.getState();
			const sources = projectSlice.selectors.selectOperandChecked(state, projectId, source)
				? projectSlice.selectors.selectCheckedOperands(state, projectId)
				: [source];
			dispatch(
				projectSlice.actions.enterKeyboardTransferMode({
					projectId,
					sources,
				}),
			);
			focusSelectionScope("outline");
		};
		const discardDiffSpec = createDiffSpec(
			change,
			diffSpecHunkHeadersForLineSelection(operand, "discard"),
		);

		const menuItemGroups: Array<Array<NativeMenuItem>> = [
			[
				preferredEditor
					? nativeMenuItem({
							label: `Open in ${preferredEditor.name}`,
							enabled: !isOpenInProgramPending,
							accelerator: toElectronAccelerator(diffHotkeys.openInEditor.hotkey),
							onSelect: () =>
								openInProgram({
									projectId,
									programId: preferredEditor.id,
									path: change.path,
									lineNr: lineNumber,
								}),
						})
					: nativeMenuItem({
							label: "Open In Editor",
							submenu:
								editors?.map((editor) =>
									nativeMenuItem({
										label: editor.name,
										enabled: !isOpenInProgramPending,
										onSelect: () =>
											openInProgram({
												projectId,
												programId: editor.id,
												path: change.path,
												lineNr: lineNumber,
											}),
									}),
								) ?? [],
						}),
				nativeMenuItem({
					label: "Copy Path",
					submenu: [
						nativeMenuItem({
							label: "Absolute Path",
							onSelect: async () => {
								const absolutePath = await window.lite.pathJoin(selectedProject.path, change.path);
								await window.lite.clipboardWriteText(absolutePath);
							},
						}),
						nativeMenuItem({
							label: "Relative Path",
							onSelect: () => window.lite.clipboardWriteText(change.path),
						}),
					],
				}),
			],
			...(operand.parent.parent._tag !== "Branch"
				? [
						[
							nativeMenuItem({
								label: "Cut Hunk",
								enabled: canUseHunk,
								onSelect: cutHunk,
								accelerator: toElectronAccelerator(selectionOperationHotkeys.cut.hotkey),
							}),
						] satisfies Array<NativeMenuItem>,
					]
				: []),
			...Match.value(operand.parent.parent).pipe(
				Match.withReturnType<Array<Array<NativeMenuItem>>>(),
				Match.when({ _tag: "Commit" }, ({ commitId }) => [
					[
						nativeMenuItem({
							label: "Uncommit",
							enabled: canUseHunk && !isCommitUncommitChangesPending,
							onSelect: () =>
								commitUncommitChanges({
									projectId,
									commitId,
									assignTo: null,
									changes: [discardDiffSpec],
									dryRun: false,
								}),
						}),
						nativeMenuItem({
							label: "Discard Changes",
							enabled: canUseHunk && !isCommitDiscardChangesPending,
							onSelect: () =>
								commitDiscardChanges({
									projectId,
									commitId,
									changes: [discardDiffSpec],
									dryRun: false,
								}),
						}),
					],
				]),
				Match.when({ _tag: "UncommittedChanges" }, () => [
					[
						nativeMenuItem({
							label: "Discard Changes",
							enabled: canUseHunk && !isDiscardWorktreeChangesPending,
							onSelect: () =>
								discardWorktreeChanges({
									projectId,
									worktreeChanges: [discardDiffSpec],
								}),
						}),
					],
				]),
				Match.orElse(() => []),
			),
		];

		return nativeMenuItemsFromGroups(menuItemGroups);
	};
};
