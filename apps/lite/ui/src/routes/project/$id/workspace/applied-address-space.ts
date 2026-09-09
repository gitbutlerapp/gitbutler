import {
	addressContains,
	addressEquals,
	addressIdentityKey,
	branchAddress,
	commitAddress,
	fileAddress,
	worktreeChangesFileParent,
	type Address,
} from "#ui/addresses.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { getOperations, type TransferKind } from "#ui/operations/operation.ts";
import { getTransferKind, type PendingOperation } from "#ui/operations/pending-operation.ts";
import { buildIndexByKey, type AddressSpace } from "#ui/workspace/address-space.ts";
import type { Stack, Worktree } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { sectionAddresses, worktreesOnTip, type Plan } from "./Graph/layout.ts";

/** A row of the list, and whether operations may take it as a source or target. */
type Row = { address: Address; owned: boolean };

const hasAnyOperation = (sources: Array<Address>, target: Address, kind: TransferKind) => {
	const operations = getOperations(sources, target, kind);
	return !!operations.into || !!operations.above || !!operations.below;
};

/**
 * The applied list's address space: the cards' rows, the worktree lanes'
 * rows where they are drawn, then the section's rows as its folds show them.
 * While an operation waits for its target only the workspace's own rows stay.
 */
export const buildAppliedAddressSpace = ({
	stacks,
	plan,
	worktreeFiles,
	pendingOperation,
	absorptionTargetCommitIds,
	foldedSegments,
}: {
	/** The cards in the graph's order, as `usePlan` gives them. */
	stacks: ReadonlyArray<Stack>;
	plan: Plan;
	/** Each linked worktree's uncommitted paths, by worktree name, in the order its lane lists them. */
	worktreeFiles: ReadonlyMap<string, ReadonlyArray<string>>;
	pendingOperation: PendingOperation;
	absorptionTargetCommitIds: ReadonlySet<string>;
	foldedSegments: Record<string, true>;
}): AddressSpace<Address> => {
	// Operations take a card's rows, a worktree's uncommitted files, and a
	// worktree's branch; not the section's rows or a worktree's own commits.
	const owned = (address: Address): Row => ({ address, owned: true });
	const foreign = (address: Address): Row => ({ address, owned: false });
	// A lane's rows in reading order: files, branch, then commits, each preceded
	// by the lanes resting on it. Matches WorktreeLane.
	const laneRows = (worktree: Worktree): Array<Row> => [
		...(worktreeFiles.get(worktree.name) ?? []).map((path) =>
			owned(fileAddress({ parent: worktreeChangesFileParent(worktree.name), path })),
		),
		...(worktree.refName
			? [owned(branchAddress({ branchRef: worktree.refName.fullNameBytes }))]
			: []),
		...worktree.commits.flatMap((commit) => [
			...lanesOn(commit.id),
			foreign(commitAddress({ commitId: commit.id, changeId: commit.changeId })),
		]),
	];
	const lanesOn = (commitId: string): Array<Row> =>
		(plan.worktrees.on.get(commitId) ?? []).flatMap(laneRows);
	const rows = (): Array<Row> => [
		...stacks.flatMap((stack) => [
			// Matches what WorkspaceLists renders: worktrees on the tip come above the
			// top branch; a folded segment hides its commits, so they are not
			// navigable, but keeps the worktree lanes resting on them.
			...worktreesOnTip(plan.worktrees, stack).flatMap(laneRows),
			...stack.segments.flatMap((segment, segmentIndex) => {
				const folded =
					segment.refName !== null &&
					foldedSegments[decodeBytes(segment.refName.fullNameBytes)] === true;
				return [
					...(segment.refName
						? [owned(branchAddress({ branchRef: segment.refName.fullNameBytes }))]
						: []),
					...segment.commits.flatMap((commit, index) => [
						...(segmentIndex === 0 && index === 0 && segment.refName !== null
							? []
							: lanesOn(commit.id)),
						...(folded
							? []
							: [owned(commitAddress({ commitId: commit.id, changeId: commit.changeId }))]),
					]),
				];
			}),
		]),
		...plan.worktrees.standalone.flatMap(laneRows),
		...sectionAddresses(plan).map(foreign),
	];
	const allItems = (): Array<Address> => rows().map((row) => row.address);
	const workspaceItems = (): Array<Address> =>
		rows().flatMap((row) => (row.owned ? [row.address] : []));

	/**
	 * While an operation is waiting for its target, only its sources and the
	 * valid targets stay navigable: invalid destinations are left out of the
	 * list rather than rejected when selected.
	 */
	const compatibleItems = ({
		sources,
		isCompatibleTarget,
	}: {
		sources: Array<Address>;
		isCompatibleTarget: (address: Address) => boolean;
	}): Array<Address> =>
		workspaceItems().filter(
			(address) =>
				sources.some(
					(source) => addressEquals(address, source) || addressContains(address, source),
				) || isCompatibleTarget(address),
		);

	const filteredItems = Match.value(pendingOperation).pipe(
		Match.tagsExhaustive({
			None: () => allItems(),
			Absorb: (operation) =>
				compatibleItems({
					sources: operation.sources,
					isCompatibleTarget: (address) =>
						address._tag === "Commit" && absorptionTargetCommitIds.has(address.commitId),
				}),
			Transfer: ({ value: operation }) =>
				compatibleItems({
					sources: operation.sources,
					isCompatibleTarget: (address) =>
						hasAnyOperation(operation.sources, address, getTransferKind(operation)),
				}),
			InlineEdit: (x) => [x.address],
		}),
	);

	const indexByKey = buildIndexByKey(filteredItems, addressIdentityKey);

	return { items: filteredItems, indexByKey };
};
