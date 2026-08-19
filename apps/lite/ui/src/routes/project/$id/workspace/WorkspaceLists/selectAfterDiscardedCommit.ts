import type { HeadInfoIndex } from "#ui/api/ref-info.ts";
import {
	branchAddress,
	commitAddress,
	addressIdentityKey,
	type CommitAddress,
	type Address,
} from "#ui/addresses.ts";
import type { AddressSpace } from "#ui/workspace/address-space.ts";

export const selectAfterDiscardedCommits = ({
	addressSpace,
	commit,
	discardedCommitIds,
	headInfoIndex,
}: {
	addressSpace: AddressSpace<Address>;
	commit: CommitAddress;
	discardedCommitIds: ReadonlySet<string>;
	headInfoIndex: HeadInfoIndex | undefined;
}): Address | null => {
	if (!discardedCommitIds.has(commit.commitId)) return commitAddress(commit);

	const commitIndex = addressSpace.indexByKey.get(addressIdentityKey(commitAddress(commit)));
	if (commitIndex === undefined) return null;

	for (let index = commitIndex + 1; ; index++) {
		const nextCommit = addressSpace.items[index];
		if (nextCommit?._tag !== "Commit") break;
		if (!discardedCommitIds.has(nextCommit.commitId)) return nextCommit;
	}

	for (let index = commitIndex - 1; ; index--) {
		const prevCommit = addressSpace.items[index];
		if (prevCommit?._tag !== "Commit") break;
		if (!discardedCommitIds.has(prevCommit.commitId)) return prevCommit;
	}

	const commitCtx = headInfoIndex?.commitContextByCommitId(commit.commitId);
	if (!commitCtx?.segment.refName) return null;

	const branchIdx = addressSpace.indexByKey.get(
		addressIdentityKey(
			branchAddress({
				branchRef: commitCtx.segment.refName.fullNameBytes,
			}),
		),
	);
	if (branchIdx === undefined) return null;

	return addressSpace.items[branchIdx] ?? null;
};
