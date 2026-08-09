import {
	useCommitUncommitChanges,
	useDiscardFileChanges,
	useOpenInProgram,
} from "#ui/api/mutations.ts";
import {
	guiSettingsQueryOptions,
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
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
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
	const dispatch = useAppDispatch();
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
	const { canDiscard, discard } = useDiscardFileChanges({
		projectId,
		fileParent: operand.parent,
	});
	// A file's actions apply to the checked set when the file is part of it, as dragging it does.
	const isChecked = useAppSelector((state) =>
		projectSlice.selectors.selectOperandChecked(state, projectId, fileOperand(operand)),
	);
	// Gated on the set being wholly ours, as the discard is, so the label can't overstate it.
	const discardFileCount = useAppSelector((state) =>
		isChecked && projectSlice.selectors.selectCanCheckFiles(state, projectId, operand.parent)
			? projectSlice.selectors.selectCheckedOperandCount(state, projectId)
			: 1,
	);
	const discardLabel =
		discardFileCount > 1 ? `Discard Changes in ${discardFileCount} Files` : "Discard Changes";
	const { isPending: isOpenInProgramPending, mutate: openInProgram } = useOpenInProgram();
	const cutFile = () => {
		dispatch(
			projectSlice.actions.enterKeyboardTransferMode({
				projectId,
				sources: [fileOperand(operand)],
			}),
		);
		focusSelectionScope("outline");
	};

	const menuItemGroups: Array<Array<NativeMenuItem>> = [
		[
			preferredEditor
				? nativeMenuItem({
						label: `Open in ${preferredEditor.name}`,
						enabled: !isOpenInProgramPending,
						accelerator: toElectronAccelerator(changesFileHotkeys.openInEditor.hotkey),
						onSelect: () =>
							openInProgram({
								projectId,
								programId: preferredEditor.id,
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
									enabled: !isOpenInProgramPending,
									onSelect: () =>
										openInProgram({
											projectId,
											programId: editor.id,
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

						return [
							[
								nativeMenuItem({
									label: "Uncommit",
									enabled: !isCommitUncommitChangesPending,
									accelerator: toElectronAccelerator(changesFileHotkeys.uncommit.hotkey),
									onSelect: uncommit,
								}),
								nativeMenuItem({
									label: discardLabel,
									enabled: canDiscard,
									accelerator: toElectronAccelerator(changesFileHotkeys.discard.hotkey),
									onSelect: () => discard({ change, extendToCheckedFiles: isChecked }),
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

						return [
							[
								nativeMenuItem({
									label: "Absorb",
									accelerator: toElectronAccelerator(changesFileHotkeys.absorb.hotkey),
									onSelect: absorb,
								}),
								nativeMenuItem({
									label: discardLabel,
									enabled: canDiscard,
									accelerator: toElectronAccelerator(changesFileHotkeys.discard.hotkey),
									onSelect: () => discard({ change, extendToCheckedFiles: isChecked }),
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
