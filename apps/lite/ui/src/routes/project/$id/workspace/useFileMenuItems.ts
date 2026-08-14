import { useDiscardFileChanges, useOpenInProgram } from "#ui/api/mutations.ts";
import { enterAbsorb, enterKeyboardTransfer } from "#ui/use-cursor.ts";
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
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector, useAppStore } from "#ui/store.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { Match } from "effect";

export const useFileMenuItems = ({
	projectId,
	operand,
	path,
	change,
	canUncommit,
	uncommit,
}: {
	projectId: string;
	operand: FileOperand;
	path: string;
	change?: TreeChange;
	canUncommit: boolean;
	uncommit?: (change: TreeChange, extendToCheckedFiles: boolean) => void;
}): Array<NativeMenuItem> => {
	const store = useAppStore();
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

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
		const source = fileOperand(operand);
		const state = store.getState();
		const sources = projectSlice.selectors.selectOperandChecked(state, projectId, source)
			? projectSlice.selectors.selectCheckedOperands(state, projectId)
			: [source];

		enterKeyboardTransfer({ sources });
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
					Match.when({ parent: { _tag: "Commit" } }, () => [
						[
							nativeMenuItem({
								label: "Uncommit",
								enabled: canUncommit,
								accelerator: toElectronAccelerator(changesFileHotkeys.uncommit.hotkey),
								onSelect: () => uncommit?.(change, isChecked),
							}),
							nativeMenuItem({
								label: discardLabel,
								enabled: canDiscard,
								accelerator: toElectronAccelerator(changesFileHotkeys.discard.hotkey),
								onSelect: () => discard({ change, extendToCheckedFiles: isChecked }),
							}),
						],
					]),
					Match.when({ parent: { _tag: "UncommittedChanges" } }, (operand) => {
						const absorb = () => {
							enterAbsorb({
								source: fileOperand(operand),
								sourceTarget: {
									type: "treeChanges",
									subject: {
										changes: [change],
										assignedStackId: null,
									},
								},
							});
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
