import { Match } from "effect";
import type { HunkLineSelection } from "#ui/hunk.ts";

export type Address =
	| { _tag: "UncommittedChanges" }
	/**
	 * Operations act only on branches applied to the workspace — `addressLabel`
	 * asserts the ref resolves to a segment. Cursors are broader: the unapplied
	 * list addresses branches outside the workspace with this same arm.
	 */
	| ({ _tag: "Branch" } & BranchAddress)
	| ({ _tag: "Commit" } & CommitAddress)
	| ({ _tag: "File" } & FileAddress)
	| ({ _tag: "Hunk" } & HunkAddress);

export type FileParent = Extract<Address, { _tag: "UncommittedChanges" | "Branch" | "Commit" }>;

export type BranchAddress = {
	branchRef: Array<number>;
};

/**
 * The commit address holds two forms of identity, the commit ID and the change ID, corresponding to
 * strong and weak identity respectively. Use one or the other as needed.
 */
export type CommitAddress = {
	commitId: string;
	changeId: string;
};

export type FileAddress = {
	parent: FileParent;
	path: string;
};

export type HunkAddress = HunkLineSelection & {
	parent: FileAddress;
	isResultOfBinaryToTextConversion: boolean;
};

export const uncommittedChangesAddress: Address = {
	_tag: "UncommittedChanges",
};

export const branchAddress = ({
	branchRef,
}: BranchAddress): Extract<Address, { _tag: "Branch" }> => ({
	_tag: "Branch",
	branchRef,
});

export const commitAddress = ({
	commitId,
	changeId,
}: CommitAddress): Extract<Address, { _tag: "Commit" }> => ({
	_tag: "Commit",
	commitId,
	changeId,
});

export const fileAddress = ({ parent, path }: FileAddress): Extract<Address, { _tag: "File" }> => ({
	_tag: "File",
	parent,
	path,
});

export const hunkAddress = ({
	parent,
	isResultOfBinaryToTextConversion,
	...lineSelection
}: HunkAddress): Extract<Address, { _tag: "Hunk" }> => ({
	_tag: "Hunk",
	parent,
	isResultOfBinaryToTextConversion,
	...lineSelection,
});

export const uncommittedChangesFileParent: FileParent = {
	_tag: "UncommittedChanges",
};

export const branchFileParent = ({ branchRef }: BranchAddress): FileParent => ({
	_tag: "Branch",
	branchRef,
});

export const commitFileParent = ({ commitId, changeId }: CommitAddress): FileParent => ({
	_tag: "Commit",
	commitId,
	changeId,
});

const uncommittedChangesIdentityKey = "uncommitted_changes";

export const branchIdentityKey = (address: BranchAddress) =>
	`branch:${address.branchRef.join(",")}`;

export const commitIdentityKey = (address: Pick<CommitAddress, "commitId">) =>
	`commit:${address.commitId}`;

export const weakCommitIdentityKey = (address: Pick<CommitAddress, "changeId">) =>
	`commit:${address.changeId}`;

const fileParentIdentityKey = (fp: FileParent): string => {
	switch (fp._tag) {
		case "UncommittedChanges":
			return uncommittedChangesIdentityKey;
		case "Branch":
			return branchIdentityKey(fp);
		case "Commit":
			return commitIdentityKey(fp);
	}
};

export const weakFileParentIdentityKey = (fp: FileParent): string => {
	switch (fp._tag) {
		case "UncommittedChanges":
			return uncommittedChangesIdentityKey;
		case "Branch":
			return branchIdentityKey(fp);
		case "Commit":
			return weakCommitIdentityKey(fp);
	}
};

const fileIdentityKey = (address: FileAddress) =>
	`file:${address.path} <- ${fileParentIdentityKey(address.parent)}`;

export const weakFileIdentityKey = (address: FileAddress) =>
	`file:${address.path} <- ${weakFileParentIdentityKey(address.parent)}`;

const hunkIdentityKey = (address: HunkAddress) =>
	`hunk:${JSON.stringify(address.hunkHeader)}:${JSON.stringify(address.lineGroups)}:${address.isResultOfBinaryToTextConversion} <- ${fileIdentityKey(address.parent)}`;

export const hunkAddressContainsLine = (source: HunkAddress, line: HunkAddress): boolean =>
	fileIdentityKey(source.parent) === fileIdentityKey(line.parent) &&
	source.isResultOfBinaryToTextConversion === line.isResultOfBinaryToTextConversion &&
	source.hunkHeader.oldStart === line.hunkHeader.oldStart &&
	source.hunkHeader.oldLines === line.hunkHeader.oldLines &&
	source.hunkHeader.newStart === line.hunkHeader.newStart &&
	source.hunkHeader.newLines === line.hunkHeader.newLines &&
	line.lineGroups.every((lineGroup) =>
		source.lineGroups.some(
			(sourceGroup) =>
				sourceGroup.side === lineGroup.side &&
				sourceGroup.start <= lineGroup.start &&
				sourceGroup.start + sourceGroup.lines >= lineGroup.start + lineGroup.lines,
		),
	);

export const addressIdentityKey = (address: Address): string => {
	switch (address._tag) {
		case "UncommittedChanges":
			return uncommittedChangesIdentityKey;
		case "File":
			return fileIdentityKey(address);
		case "Branch":
			return branchIdentityKey(address);
		case "Commit":
			return commitIdentityKey(address);
		case "Hunk":
			return hunkIdentityKey(address);
	}
};

export const addressEquals = (a: Address, b: Address): boolean =>
	addressIdentityKey(a) === addressIdentityKey(b);

export const addressFileParent = (address: Address): FileParent | null =>
	Match.value(address).pipe(
		Match.withReturnType<FileParent | null>(),
		Match.tags({
			File: ({ parent }) => parent,
			UncommittedChanges: () => uncommittedChangesAddress,
			Hunk: ({ parent }) => parent.parent,
		}),
		Match.orElse(() => null),
	);

export const addressContains = (a: Address, b: Address) => {
	const bFileParent = addressFileParent(b);
	return bFileParent && addressEquals(a, bFileParent);
};
