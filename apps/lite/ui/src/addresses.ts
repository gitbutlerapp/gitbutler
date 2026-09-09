import { Match } from "effect";
import type { HunkLineSelection } from "#ui/hunk.ts";
import type { ChangesSource } from "@gitbutler/but-sdk";

export type Address =
	| ({ _tag: "UncommittedChanges" } & UncommittedChangesAddress)
	/**
	 * Operations act on branches applied to the workspace, and on a branch checked
	 * out in a linked worktree as a target. Cursors are broader still: the
	 * unapplied list addresses branches outside the workspace with this same arm.
	 */
	| ({ _tag: "Branch" } & BranchAddress)
	| ({ _tag: "Commit" } & CommitAddress)
	| ({ _tag: "File" } & FileAddress)
	| ({ _tag: "Hunk" } & HunkAddress);

export type FileParent = Extract<Address, { _tag: "UncommittedChanges" | "Branch" | "Commit" }>;

/** The uncommitted changes of the main worktree, or of the linked worktree named. */
export type UncommittedChangesAddress = {
	/** The stable worktree name, i.e. the directory under `$GIT_COMMON_DIR/worktrees/`. */
	worktree?: string;
};

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

/** The checkout an uncommitted-changes address reads from, as the API names it. */
export const changesSourceOf = ({ worktree }: UncommittedChangesAddress): ChangesSource =>
	worktree === undefined ? { type: "head" } : { type: "worktree", subject: worktree };

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

export const uncommittedChangesFileParent: Extract<FileParent, { _tag: "UncommittedChanges" }> = {
	_tag: "UncommittedChanges",
};

export const worktreeChangesFileParent = (
	worktree: string,
): Extract<FileParent, { _tag: "UncommittedChanges" }> => ({
	_tag: "UncommittedChanges",
	worktree,
});

export const branchFileParent = ({ branchRef }: BranchAddress): FileParent => ({
	_tag: "Branch",
	branchRef,
});

export const commitFileParent = ({ commitId, changeId }: CommitAddress): FileParent => ({
	_tag: "Commit",
	commitId,
	changeId,
});

const uncommittedChangesIdentityKey = ({ worktree }: UncommittedChangesAddress) =>
	worktree === undefined ? "uncommitted_changes" : `uncommitted_changes:${worktree}`;

export const branchIdentityKey = (address: BranchAddress) =>
	`branch:${address.branchRef.join(",")}`;

export const commitIdentityKey = (address: Pick<CommitAddress, "commitId">) =>
	`commit:${address.commitId}`;

export const weakCommitIdentityKey = (address: Pick<CommitAddress, "changeId">) =>
	`commit:${address.changeId}`;

const fileParentIdentityKey = (fp: FileParent): string => {
	switch (fp._tag) {
		case "UncommittedChanges":
			return uncommittedChangesIdentityKey(fp);
		case "Branch":
			return branchIdentityKey(fp);
		case "Commit":
			return commitIdentityKey(fp);
	}
};

export const weakFileParentIdentityKey = (fp: FileParent): string => {
	switch (fp._tag) {
		case "UncommittedChanges":
			return uncommittedChangesIdentityKey(fp);
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
			return uncommittedChangesIdentityKey(address);
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
			UncommittedChanges: (address) => address,
			Hunk: ({ parent }) => parent.parent,
		}),
		Match.orElse(() => null),
	);

export const addressContains = (a: Address, b: Address) => {
	const bFileParent = addressFileParent(b);
	return bFileParent && addressEquals(a, bFileParent);
};
