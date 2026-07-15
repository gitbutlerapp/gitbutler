import {
	useCommitDiscardChanges,
	useCommitUncommitChanges,
	useDiscardWorktreeChanges,
	useOpenInEditor,
} from "#ui/api/mutations.ts";
import {
	getGUISettingsQueryOptions,
	listEditorsQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import {
	changesFileHotkeys,
	selectionOperationHotkeys,
	toElectronAccelerator,
} from "#ui/hotkeys.ts";
import { type NativeMenuItem, nativeMenuItem, nativeMenuItemsFromGroups } from "#ui/native-menu.ts";
import { fileOperand, type FileOperand } from "#ui/operands.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { useProjectStore } from "#ui/store.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
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
	const projectStore = useProjectStore(projectId);
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...getGUISettingsQueryOptions(),
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
	const { isPending: isOpenInEditorPending, mutate: openInEditor } = useOpenInEditor();
	const cutFile = () => {
		projectStore.enterKeyboardTransferMode(fileOperand(operand));
		focusSelectionScope("outline");
	};

	const menuItemGroups: Array<Array<NativeMenuItem>> = [
		[
			preferredEditor
				? nativeMenuItem({
						label: `Open in ${preferredEditor.name}`,
						enabled: !isOpenInEditorPending,
						accelerator: toElectronAccelerator(changesFileHotkeys.openInEditor.hotkey),
						onSelect: () =>
							openInEditor({
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
									enabled: !isOpenInEditorPending,
									onSelect: () =>
										openInEditor({
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
							commitUncommitChanges({
								projectId,
								commitId: operand.parent.commitId,
								assignTo: null,
								changes: [createDiffSpec(change, [])],
								dryRun: false,
							});
						const discard = () =>
							commitDiscardChanges({
								projectId,
								commitId: operand.parent.commitId,
								changes: [createDiffSpec(change, [])],
								dryRun: false,
							});

						return [
							[
								nativeMenuItem({
									label: "Uncommit",
									enabled: !isCommitUncommitChangesPending,
									onSelect: uncommit,
								}),
								nativeMenuItem({
									label: "Discard Changes",
									enabled: !isCommitDiscardChangesPending,
									onSelect: discard,
								}),
							],
						];
					}),
					Match.when({ parent: { _tag: "UncommittedChanges" } }, (operand) => {
						const absorb = () => {
							projectStore.enterAbsorbMode(fileOperand(operand), {
								type: "treeChanges",
								subject: {
									changes: [change],
									assignedStackId: null,
								},
							});
							focusSelectionScope("outline");
						};
						const discard = () =>
							discardWorktreeChanges({
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
									enabled: !isDiscardWorktreeChangesPending,
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
