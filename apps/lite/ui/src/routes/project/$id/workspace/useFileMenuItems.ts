import {
	useCommitDiscardChanges,
	useCommitUncommitChanges,
	useDiscardWorktreeChanges,
	useOpenInEditor,
} from "#ui/api/mutations.ts";
import {
	useGetGUISettingsQuery,
	useListEditorsQuery,
	useListProjectsQuery,
} from "#ui/api/queries.ts";
import {
	changesFileHotkeys,
	selectionOperationHotkeys,
	toElectronAccelerator,
} from "#ui/hotkeys.ts";
import { type NativeMenuItem, nativeMenuItem, nativeMenuItemsFromGroups } from "#ui/native-menu.ts";
import { fileOperand, type FileOperand } from "#ui/operands.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
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
	const { data: projects = [] } = useListProjectsQuery();
	const { data: editors } = useListEditorsQuery();
	const { data: settings } = useGetGUISettingsQuery();
	const preferredEditor = editors?.find((editor) => editor.id === settings?.editorId);

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	const commitUncommitChanges = useCommitUncommitChanges();
	const commitDiscardChanges = useCommitDiscardChanges();
	const discardWorktreeChanges = useDiscardWorktreeChanges();
	const openInEditor = useOpenInEditor();
	const cutFile = () => {
		dispatch(
			projectSlice.actions.enterKeyboardTransferMode({
				projectId,
				source: fileOperand(operand),
			}),
		);
		focusSelectionScope("outline");
	};

	const menuItemGroups: Array<Array<NativeMenuItem>> = [
		[
			preferredEditor
				? nativeMenuItem({
						label: `Open in ${preferredEditor.name}`,
						enabled: !openInEditor.isPending,
						accelerator: toElectronAccelerator(changesFileHotkeys.openInEditor.hotkey),
						onSelect: () =>
							openInEditor.mutate({
								projectId,
								editorId: preferredEditor.id,
								path,
								lineNr: null,
							}),
					})
				: nativeMenuItem({
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
					] satisfies Array<NativeMenuItem>,
				]
			: []),
		...(change
			? Match.value(operand).pipe(
					Match.withReturnType<Array<Array<NativeMenuItem>>>(),
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
								projectSlice.actions.enterAbsorbMode({
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
