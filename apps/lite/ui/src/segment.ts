import { decodeBytes } from "./api/bytes.ts";
import type { HeadInfoIndex } from "./api/ref-info.ts";
import type { BranchOperand } from "./operands";

/**
 * Map from old to new branch names. Helpful for resolving branches following renames. Not
 * oplog-aware.
 */
const renamedBranches = new Map<string, Array<number>>();

export const cacheRenamedBranch = (oldName: Array<number>, newName: Array<number>): void => {
	renamedBranches.set(decodeBytes(oldName), newName);
};

/**
 * Resolve a branch whose identity has potentially changed. Attempts to resolve by branch name and
 * branch renames. O(1) (excl/ bytes encode/decode)
 */
export const resolveBranch = (
	{ branchContextByRefBytes }: HeadInfoIndex,
	selection: BranchOperand,
): BranchOperand | null => {
	const direct = branchContextByRefBytes(selection.branchRef);
	if (direct) {
		return {
			// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
			stackId: direct.stack.id!,
			branchRef: selection.branchRef,
		};
	}

	const rename = renamedBranches.get(decodeBytes(selection.branchRef));
	const ctx = rename !== undefined ? branchContextByRefBytes(rename) : null;

	return rename && ctx
		? {
				// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
				stackId: ctx.stack.id!,
				branchRef: rename,
			}
		: null;
};
