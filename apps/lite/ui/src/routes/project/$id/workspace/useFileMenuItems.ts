import { useDiscardFileChanges, useResolveWorktreeConflicts } from "#ui/api/mutations.ts";
import { startAbsorb, startKeyboardTransfer } from "#ui/use-cursor.ts";
import { changesInWorktreeQueryOptions } from "#ui/api/queries.ts";
import {
	changesFileHotkeys,
	selectionOperationHotkeys,
	toElectronAccelerator,
} from "#ui/hotkeys.ts";
import { type NativeMenuItem, nativeMenuItem, nativeMenuItemsFromGroups } from "#ui/native-menu.ts";
import { usePathMenuItems } from "./usePathMenuItems.ts";
import { fileAddress, type FileAddress } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector, useAppStore } from "#ui/store.ts";
import { focusScope } from "#ui/focus-scopes.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { useQueryClient } from "@tanstack/react-query";
import { Match } from "effect";

export const useFileMenuItems = ({
	projectId,
	address,
	path,
	change,
	canUncommit,
	uncommit,
}: {
	projectId: string;
	address: FileAddress;
	path: string;
	change?: TreeChange;
	canUncommit: boolean;
	uncommit?: (change: TreeChange, extendToCheckedFiles: boolean) => void;
}): Array<NativeMenuItem> => {
	const store = useAppStore();
	const queryClient = useQueryClient();
	const pathMenuItems = usePathMenuItems({ projectId, path });

	const { canDiscard, discard } = useDiscardFileChanges({
		projectId,
		fileParent: address.parent,
	});
	// A file's actions apply to the checked set when the file is part of it, as dragging it does.
	const isChecked = useAppSelector((state) =>
		projectSlice.selectors.selectAddressChecked(state, projectId, fileAddress(address)),
	);
	// Gated on the set being wholly ours, as the discard is, so the label can't overstate it.
	const discardFileCount = useAppSelector((state) =>
		isChecked && projectSlice.selectors.selectCanCheckFiles(state, projectId, address.parent)
			? projectSlice.selectors.selectCheckedAddressCount(state, projectId)
			: 1,
	);
	const discardLabel =
		discardFileCount > 1 ? `Discard Changes in ${discardFileCount} Files` : "Discard Changes";
	const { isPending: isResolvePending, mutate: resolveWorktreeConflicts } =
		useResolveWorktreeConflicts();
	// A file listed under uncommitted changes without a change is a conflicted one.
	const isWorktreeConflict = !change && address.parent._tag === "UncommittedChanges";
	const cutFile = () => {
		const source = fileAddress(address);
		const state = store.getState();
		const sources = projectSlice.selectors.selectAddressChecked(state, projectId, source)
			? projectSlice.selectors.selectCheckedAddresses(state, projectId)
			: [source];

		startKeyboardTransfer({ sources, kind: "move" });
		focusScope("sidebar");
	};

	const menuItemGroups: Array<Array<NativeMenuItem>> = [
		pathMenuItems,
		...(isWorktreeConflict
			? [
					[
						nativeMenuItem({
							label: "Mark as Resolved",
							enabled: !isResolvePending,
							onSelect: () => resolveWorktreeConflicts({ projectId, paths: [path] }),
						}),
					] satisfies Array<NativeMenuItem>,
				]
			: []),
		...(change && address.parent._tag !== "Branch"
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
			? Match.value(address).pipe(
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
					Match.when({ parent: { _tag: "UncommittedChanges" } }, (address) => {
						const absorb = () => {
							const source = fileAddress(address);
							const state = store.getState();
							const checkedPaths = projectSlice.selectors.selectAddressChecked(
								state,
								projectId,
								source,
							)
								? projectSlice.selectors.selectCheckedUncommittedFilePaths(state, projectId)
								: new Set<string>();
							const checkedChanges =
								checkedPaths.size === 0
									? null
									: queryClient
											.getQueryData(changesInWorktreeQueryOptions(projectId).queryKey)
											?.changes.filter((change) => checkedPaths.has(change.path));
							if (checkedPaths.size > 0 && !checkedChanges) return;

							startAbsorb({
								sources: (checkedPaths.size > 0
									? Array.from(checkedPaths, (path) =>
											fileAddress({ parent: address.parent, path }),
										)
									: null) ?? [source],
								sourceTarget: {
									type: "treeChanges",
									subject: {
										changes: checkedChanges ?? [change],
										assignedStackId: null,
									},
								},
							});
							focusScope("sidebar");
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
