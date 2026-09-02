import type { HeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitTitle, shortCommitId } from "#ui/commit.ts";
import { Match } from "effect";
import type { Address } from "#ui/addresses.ts";
import { assert } from "#ui/assert.ts";

/** What a set of hunk lines amounts to: "+3 -1 lines", the words a drag of them carries. */
const hunkAddressesLabel = (addresses: Array<Extract<Address, { _tag: "Hunk" }>>) => {
	const add = addresses.reduce(
		(sum, address) =>
			sum +
			address.lineGroups.reduce(
				(groupSum, group) => groupSum + (group.side === "additions" ? group.lines : 0),
				0,
			),
		0,
	);
	const del = addresses.reduce(
		(sum, address) =>
			sum +
			address.lineGroups.reduce(
				(groupSum, group) => groupSum + (group.side === "deletions" ? group.lines : 0),
				0,
			),
		0,
	);
	const all = add + del;

	// Probably shouldn't happen?
	if (all == 0) return "0 changed lines";

	let words = "";
	if (add > 0) words += `+${add}`;
	if (add > 0 && del > 0) words += ` `;
	if (del > 0) words += `-${del}`;
	if (all === 1) words += " line";
	else words += " lines";
	return words;
};

export const addressLabel = ({
	address,
	headInfoIndex,
}: {
	address: Address;
	headInfoIndex: HeadInfoIndex;
}) =>
	Match.value(address).pipe(
		Match.tagsExhaustive({
			Branch: ({ branchRef }) => {
				const segment = headInfoIndex.branchContextByRefBytes(branchRef)?.segment;
				return assert(segment?.refName).displayName;
			},
			File: ({ path }) => path,
			UncommittedChanges: () => "Uncommitted changes",
			Commit: ({ commitId }) => {
				const commit = headInfoIndex.commitContextByCommitId(commitId)?.commit;
				return commit
					? `${commitTitle(commit.message) ?? "(no message)"}${commit.hasConflicts ? " ⚠️" : ""}`
					: shortCommitId(commitId);
			},
			Hunk: (address) => hunkAddressesLabel([address]),
		}),
	);

export const addressesLabel = ({
	addresses,
	headInfoIndex,
}: {
	addresses: Array<Address>;
	headInfoIndex: HeadInfoIndex;
}) => {
	if (addresses.length > 0 && addresses.every((address) => address._tag === "Hunk"))
		return hunkAddressesLabel(addresses);
	if (addresses.length !== 1) return `${addresses.length.toLocaleString()} items`;

	return addressLabel({ address: assert(addresses[0]), headInfoIndex });
};
