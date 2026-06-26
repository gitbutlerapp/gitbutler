import {
	useCommitDiscardChanges,
	useCommitUncommitChanges,
	useDiscardWorktreeChanges,
	useOpenInEditor,
} from "#ui/api/mutations.ts";
import { listEditorsQueryOptions, listProjectsQueryOptions } from "#ui/api/queries.ts";
import {
	changesFileHotkeys,
	selectionOperationHotkeys,
	toElectronAccelerator,
} from "#ui/hotkeys.ts";
import { type NativeMenuItem, nativeMenuItem, nativeMenuItemsFromGroups } from "#ui/native-menu.ts";
import { fileOperand, type FileOperand } from "#ui/operands.ts";
import { keyboardTransferOperationMode } from "#ui/outline/mode.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { projectActions } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import type { NonEmptyArray } from "effect/Array";
import { Match } from "effect";

export const useFileMenuItems = ({
	projectId,
	operand,
	path,
	change,
}: {
	projectId: string;
	operand: FileOperand;
	path: string;
	change?: TreeChange;
}): Array<NativeMenuItem> => {
	const dispatch = useAppDispatch();
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const { data: editors } = useQuery(listEditorsQueryOptions);

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	const commitUncommitChanges = useCommitUncommitChanges();
	const commitDiscardChanges = useCommitDiscardChanges();
	const discardWorktreeChanges = useDiscardWorktreeChanges();
	const openInEditor = useOpenInEditor();
	const cutFile = () => {
		dispatch(
			projectActions.enterTransferMode({
				projectId,
				mode: keyboardTransferOperationMode({
					source: fileOperand(operand),
					operationType: "into",
				}),
			}),
		);
		focusSelectionScope("outline");
	};

	const menuItemGroups: Array<NonEmptyArray<NativeMenuItem>> = [
		[
			nativeMenuItem({
				label: "Open In Editor",
				submenu:
					editors?.map((editor) =>
						nativeMenuItem({
							label: editor.name,
							enabled: !openInEditor.isPending,
							onSelect: () =>
								openInEditor.mutate({
									projectId,
									editorId: editor.id,
									path,
									lineNr: null,
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
							const absolutePath = await window.lite.pathJoin(selectedProject.path, path);
							await window.lite.clipboardWriteText(absolutePath);
						},
					}),
					nativeMenuItem({
						label: "Relative Path",
						onSelect: () => window.lite.clipboardWriteText(path),
					}),
				],
			}),
		],
		...(change && operand.parent._tag !== "Branch"
			? [
					[
						nativeMenuItem({
							label: "Cut File",
							onSelect: cutFile,
							accelerator: toElectronAccelerator(selectionOperationHotkeys.cut.hotkey),
						}),
					] satisfies NonEmptyArray<NativeMenuItem>,
				]
			: []),
		...(change
			? Match.value(operand).pipe(
					Match.withReturnType<Array<NonEmptyArray<NativeMenuItem>>>(),
					Match.when({ parent: { _tag: "Commit" } }, (operand) => {
						const uncommit = () =>
							commitUncommitChanges.mutate({
								projectId,
								commitId: operand.parent.commitId,
								assignTo: null,
								changes: [createDiffSpec(change, [])],
								dryRun: false,
							});
						const discard = () =>
							commitDiscardChanges.mutate({
								projectId,
								commitId: operand.parent.commitId,
								changes: [createDiffSpec(change, [])],
								dryRun: false,
							});

						return [
							[
								nativeMenuItem({
									label: "Uncommit",
									enabled: !commitUncommitChanges.isPending,
									onSelect: uncommit,
								}),
								nativeMenuItem({
									label: "Discard Changes",
									enabled: !commitDiscardChanges.isPending,
									onSelect: discard,
								}),
							],
						];
					}),
					Match.when({ parent: { _tag: "UncommittedChanges" } }, (operand) => {
						const absorb = () => {
							dispatch(
								projectActions.enterAbsorbMode({
									projectId,
									source: fileOperand(operand),
									sourceTarget: {
										type: "treeChanges",
										subject: {
											changes: [change],
											assignedStackId: null,
										},
									},
								}),
							);
							focusSelectionScope("outline");
						};
						const discard = () =>
							discardWorktreeChanges.mutate({
								projectId,
								changes: [createDiffSpec(change, [])],
							});

						return [
							[
								nativeMenuItem({
									label: "Absorb",
									accelerator: toElectronAccelerator(changesFileHotkeys.absorb.hotkey),
									onSelect: absorb,
								}),
								nativeMenuItem({
									label: "Discard Changes",
									enabled: !discardWorktreeChanges.isPending,
									onSelect: discard,
								}),
							],
						];
					}),
					Match.orElse(() => []),
				)
			: []),
	];

	return nativeMenuItemsFromGroups(menuItemGroups);
};
