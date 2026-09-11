import {
	useCommitDiscardChanges,
	useCommitUncommitChanges,
	useDiscardWorktreeChanges,
} from "#ui/api/mutations.ts";
import { Icon } from "#ui/components/Icon.tsx";
import {
	nativeMenuItem,
	nativeMenuItemsFromGroups,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import type { FileParent } from "#ui/addresses.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { Toolbar } from "@base-ui/react";
import type { TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";
import type { FC } from "react";
import { FileStatusBadge, type FileStatusType } from "#ui/components/FileStatusBadge.tsx";
import styles from "./ChangesHeaderRow.module.css";
import { getRowButtonClassName } from "./Row-utils.ts";
import { RowToolbar, SectionHeaderRow } from "./Row.tsx";
import { useFileDisplayModeMenuItems } from "./useFileDisplayModeMenuItems.ts";

export const ChangesHeaderRow: FC<{
	projectId: string;
	fileParent: FileParent;
	changes: Array<TreeChange>;
	className?: string;
	onOpenFilter: () => void;
}> = ({ projectId, fileParent, changes, className, onOpenFilter }) => {
	const { isPending: isCommitUncommitChangesPending, mutate: commitUncommitChanges } =
		useCommitUncommitChanges();
	const { isPending: isCommitDiscardChangesPending, mutate: commitDiscardChanges } =
		useCommitDiscardChanges();
	const { isPending: isDiscardWorktreeChangesPending, mutate: discardWorktreeChanges } =
		useDiscardWorktreeChanges();

	const counts: Partial<Record<FileStatusType, number>> = {};
	for (const change of changes) counts[change.status.type] = (counts[change.status.type] ?? 0) + 1;

	const diffSpecs = () => changes.map((change) => createDiffSpec(change, []));
	const fileDisplayModeMenuItems = useFileDisplayModeMenuItems();

	const menuItems: Array<NativeMenuItem> = nativeMenuItemsFromGroups([
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

	return (
		<div className={className}>
			<SectionHeaderRow
				label="Changes"
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
				actions={
					<Toolbar.Root aria-label="Changes actions" render={<RowToolbar forceVisible />}>
						{changes.length > 0 && (
							<Toolbar.Button
								aria-label="Filter files"
								onClick={onOpenFilter}
								className={getRowButtonClassName({ size: "regular", iconOnly: true })}
							>
								<Icon name="search" />
							</Toolbar.Button>
						)}

						<Toolbar.Button
							aria-label="Changes menu"
							onClick={(event) => {
								void showNativeMenuFromTrigger(event.currentTarget, menuItems);
							}}
							className={getRowButtonClassName({ size: "regular", iconOnly: true })}
						>
							<Icon name="kebab" />
						</Toolbar.Button>
					</Toolbar.Root>
				}
			/>
			<div className={styles.summary}>
				<span>{changes.length} files</span>
				{(["Modification", "Addition", "Deletion", "Rename"] satisfies Array<FileStatusType>).map(
					(status) =>
						(counts[status] ?? 0) > 0 ? (
							<span key={status} className={styles.count} data-status={status}>
								<span aria-hidden="true">·</span> {counts[status]}{" "}
								<FileStatusBadge status={status} fontSize={11} />
							</span>
						) : null,
				)}
			</div>
		</div>
	);
};
