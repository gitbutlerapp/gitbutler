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
import type { FileParent } from "#ui/operands.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { Toolbar } from "@base-ui/react";
import type { TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";
import type { FC } from "react";
import { ChangeStats } from "./ChangeStats.tsx";
import type { LineStats } from "./lineStats.ts";
import { getRowButtonClassName } from "./Row-utils.ts";
import { RowToolbar, SectionHeaderRow } from "./Row.tsx";

export const ChangesHeaderRow: FC<{
	projectId: string;
	fileParent: FileParent;
	changes: Array<TreeChange>;
	lineStats: LineStats;
	className?: string;
	onOpenFilter: () => void;
}> = ({ projectId, fileParent, changes, lineStats, className, onOpenFilter }) => {
	const { isPending: isCommitUncommitChangesPending, mutate: commitUncommitChanges } =
		useCommitUncommitChanges();
	const { isPending: isCommitDiscardChangesPending, mutate: commitDiscardChanges } =
		useCommitDiscardChanges();
	const { isPending: isDiscardWorktreeChangesPending, mutate: discardWorktreeChanges } =
		useDiscardWorktreeChanges();

	const diffSpecs = () => changes.map((change) => createDiffSpec(change, []));

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
							onSelect: () => discardWorktreeChanges({ projectId, changes: diffSpecs() }),
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
	]);

	return (
		<SectionHeaderRow
			label="Changes"
			className={className}
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
		>
			<ChangeStats fileCount={changes.length} lineStats={lineStats} />
		</SectionHeaderRow>
	);
};
