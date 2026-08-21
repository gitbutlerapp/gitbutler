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
import type { RefInfo } from "@gitbutler/but-sdk";
import { Match } from "effect";

const hasAnyOperation = (sources: Array<Address>, target: Address, kind: TransferKind) => {
	const operations = getOperations(sources, target, kind);
	return !!operations.into || !!operations.above || !!operations.below;
};

/**
 * The applied list's address space, shared by every host that renders the
 * workspace lists.
 */
export const buildAppliedAddressSpace = ({
	headInfo,
	pendingOperation,
	absorptionTargetCommitIds,
	foldedSegments,
}: {
	headInfo: RefInfo | undefined;
	pendingOperation: PendingOperation;
	absorptionTargetCommitIds: ReadonlySet<string>;
	foldedSegments: Record<string, true>;
}): AddressSpace<Address> => {
	const allItems = (): Array<Address> =>
		headInfo?.stacks.toReversed().flatMap((stack) =>
			stack.segments.flatMap((segment): Array<Address> => {
				// Matches what WorkspaceLists renders: a folded segment shows a stub
				// in place of its commits, so they are not navigable.
				const folded =
					segment.refName !== null &&
					foldedSegments[decodeBytes(segment.refName.fullNameBytes)] === true;

				return [
					...(segment.refName ? [branchAddress({ branchRef: segment.refName.fullNameBytes })] : []),
					...(folded
						? []
						: segment.commits.map((commit) =>
								commitAddress({ commitId: commit.id, changeId: commit.changeId }),
							)),
				];
			}),
		) ?? [];

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
		allItems().filter(
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
