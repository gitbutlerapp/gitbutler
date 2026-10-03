import { assert } from "#ui/assert.ts";
import type { BranchCommitFilter } from "#ui/projects/project.ts";

type FilterCommit = { id: string; state: { type: string } };

/** The commits a filter shows, as indices into the newest-first list: `[newest, oldest]`. */
type Span = readonly [number, number];

const all: BranchCommitFilter = { _tag: "All" };

/** The unpushed commits are the run at the tip that the remote hasn't got. */
const unpushedSpan = (commits: ReadonlyArray<FilterCommit>): Span | null => {
	const count = commits.findIndex((commit) => commit.state.type !== "LocalOnly");
	const unpushed = count === -1 ? commits.length : count;
	return unpushed === 0 ? null : [0, unpushed - 1];
};

export const unpushedCount = (commits: ReadonlyArray<FilterCommit>): number => {
	const span = unpushedSpan(commits);
	return span === null ? 0 : span[1] + 1;
};

/**
 * The span a filter covers in `commits`, or `null` for the whole branch: also when the filter no
 * longer fits the commits, as after a rebase rewrote the ids it named.
 */
export const filterSpan = (
	filter: BranchCommitFilter,
	commits: ReadonlyArray<FilterCommit>,
): Span | null => {
	switch (filter._tag) {
		case "All":
			return null;
		case "Unpushed": {
			const span = unpushedSpan(commits);
			return span !== null && span[1] < commits.length - 1 ? span : null;
		}
		case "Range": {
			const newest = commits.findIndex((commit) => commit.id === filter.newest);
			const oldest = commits.findIndex((commit) => commit.id === filter.oldest);
			if (newest === -1 || oldest === -1 || newest > oldest) return null;
			return newest === 0 && oldest === commits.length - 1 ? null : [newest, oldest];
		}
	}
};

const fromSpan = (commits: ReadonlyArray<FilterCommit>, [newest, oldest]: Span) =>
	newest === 0 && oldest === commits.length - 1
		? all
		: ({
				_tag: "Range",
				newest: assert(commits[newest]).id,
				oldest: assert(commits[oldest]).id,
			} as const);

/**
 * Ticks or unticks one commit, keeping the selection an unbroken run, since only a run has a diff
 * of its own. Ticking outside the run stretches it to reach the commit. Unticking an end shrinks
 * the run by one; unticking inside it drops that commit and the older ones. Unticking the last
 * one, or ticking every commit, shows the whole branch again.
 */
export const toggleCommit = (
	filter: BranchCommitFilter,
	commits: ReadonlyArray<FilterCommit>,
	commitId: string,
): BranchCommitFilter => {
	const index = commits.findIndex((commit) => commit.id === commitId);
	if (index === -1) return filter;

	const span = filter._tag === "Range" ? filterSpan(filter, commits) : null;
	if (span === null) return fromSpan(commits, [index, index]);

	const [newest, oldest] = span;
	if (index < newest || index > oldest)
		return fromSpan(commits, [Math.min(index, newest), Math.max(index, oldest)]);
	if (newest === oldest) return all;
	if (index === oldest) return fromSpan(commits, [newest, oldest - 1]);
	if (index === newest) return fromSpan(commits, [newest + 1, oldest]);
	return fromSpan(commits, [newest, index - 1]);
};
