import {
	useCommitDiscardChanges,
	useCommitUncommitChanges,
	useDiscardWorktreeChanges,
} from "#ui/api/mutations.ts";
import { nativeMenuItem, nativeMenuItemsFromGroups, type NativeMenuItem } from "#ui/native-menu.ts";
import type { FileParent } from "#ui/addresses.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { useFileDisplayModeMenuItems } from "./useFileDisplayModeMenuItems.ts";

/** The Changes panel's menu, from its kebab or a right-click on its header. */
export const useChangesMenuItems = ({
	projectId,
	fileParent,
	changes,
}: {
	projectId: string;
	fileParent: FileParent;
	changes: Array<TreeChange>;
}): Array<NativeMenuItem> => {
	const { isPending: isCommitUncommitChangesPending, mutate: commitUncommitChanges } =
		useCommitUncommitChanges();
	const { isPending: isCommitDiscardChangesPending, mutate: commitDiscardChanges } =
		useCommitDiscardChanges();
	const { isPending: isDiscardWorktreeChangesPending, mutate: discardWorktreeChanges } =
		useDiscardWorktreeChanges();

	const diffSpecs = () => changes.map((change) => createDiffSpec(change, []));
	const fileDisplayModeMenuItems = useFileDisplayModeMenuItems();

	const menuItems = nativeMenuItemsFromGroups([
		...Match.value(fileParent).pipe(
			Match.withReturnType<Array<Array<NativeMenuItem>>>(),
			Match.tags({
				Commit: ({ commitId }) => [
					[
						nativeMenuItem({
							label: "Uncommit All",
							enabled: changes.length > 0 && !isCommitUncommitChangesPending,
							onSelect: () =>
								commitUncommitChanges({
									projectId,
									commitId,
									assignTo: null,
									changes: diffSpecs(),
									dryRun: false,
								}),
						}),
						nativeMenuItem({
							label: "Discard All Changes",
							enabled: changes.length > 0 && !isCommitDiscardChangesPending,
							onSelect: () =>
								commitDiscardChanges({
									projectId,
									commitId,
									changes: diffSpecs(),
									dryRun: false,
								}),
						}),
					],
				],
				UncommittedChanges: () => [
					[
						nativeMenuItem({
							label: "Discard Changes",
							enabled: changes.length > 0 && !isDiscardWorktreeChangesPending,
							onSelect: () => discardWorktreeChanges({ projectId, worktreeChanges: diffSpecs() }),
						}),
					],
				],
				Branch: () => [],
			}),
			Match.exhaustive,
		),
		[
			nativeMenuItem({
				label: "Copy File Paths",
				enabled: changes.length > 0,
				onSelect: () =>
					window.lite.clipboardWriteText(changes.map((change) => change.path).join("\n")),
			}),
		],
		fileDisplayModeMenuItems,
	]);

	return menuItems;
};
