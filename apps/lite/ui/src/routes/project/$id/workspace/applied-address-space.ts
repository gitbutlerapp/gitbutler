import {
	addressContains,
	addressEquals,
	addressIdentityKey,
	branchAddress,
	commitAddress,
	type Address,
} from "#ui/addresses.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { getOperations, type TransferKind } from "#ui/operations/operation.ts";
import { getTransferKind, type PendingOperation } from "#ui/operations/pending-operation.ts";
import { buildIndexByKey, type AddressSpace } from "#ui/workspace/address-space.ts";
import type { Stack } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { sectionAddresses, type Plan } from "./Graph/layout.ts";

const hasAnyOperation = (sources: Array<Address>, target: Address, kind: TransferKind) => {
	const operations = getOperations(sources, target, kind);
	return !!operations.into || !!operations.above || !!operations.below;
};

/**
 * The applied list's address space: everything the stacks column shows, top
 * to bottom, as values. Cards in the graph's order, then the upstream
 * section's rows as its folds show them. While an operation waits for its
 * target only the workspace's own rows stay: what the target holds can be
 * looked at, not acted on.
 */
export const buildAppliedAddressSpace = ({
	stacks,
	plan,
	pendingOperation,
	absorptionTargetCommitIds,
	foldedSegments,
}: {
	/** The cards in the graph's order, as `usePlan` gives them. */
	stacks: ReadonlyArray<Stack>;
	plan: Plan;
	pendingOperation: PendingOperation;
	absorptionTargetCommitIds: ReadonlySet<string>;
	foldedSegments: Record<string, true>;
}): AddressSpace<Address> => {
	// Every row as a value, tagged with whether the workspace owns it: a card's
	// branch and commit rows do, the section's do not.
	const rows = (): Array<{ address: Address; owned: boolean }> => {
		const owned = (address: Address) => ({ address, owned: true });
		const foreign = (address: Address) => ({ address, owned: false });
		return [
			...stacks.flatMap((stack) =>
				stack.segments.flatMap((segment) => {
					// Matches what WorkspaceLists renders: a folded segment shows a stub
					// in place of its commits, so they are not navigable.
					const folded =
						segment.refName !== null &&
						foldedSegments[decodeBytes(segment.refName.fullNameBytes)] === true;
					return [
						...(segment.refName
							? [owned(branchAddress({ branchRef: segment.refName.fullNameBytes }))]
							: []),
						...(folded
							? []
							: segment.commits.map((commit) =>
									owned(commitAddress({ commitId: commit.id, changeId: commit.changeId })),
								)),
					];
				}),
			),
			...sectionAddresses(plan).map(foreign),
		];
	};
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
