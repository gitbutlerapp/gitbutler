import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { useNow } from "#ui/components/useNow.ts";
import { formatRelativeTime } from "#ui/time.ts";
import type { RefInfo } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import type { FC } from "react";
import styles from "./CleanWorktreeNote.module.css";

type LastCommit = { committedAt: number; branch: string | null };

/**
 * The most recently committed commit across every applied stack, and the branch
 * it went to.
 *
 * Newest by commit time rather than the checked-out branch's tip: with several
 * stacks applied, the one just worked on is the one the question is about.
 * Commits in a branchless segment belong to the first named branch below it,
 * which is where the rows below would show them.
 *
 * Module level, and passed to `select` by reference, so react-query can cache
 * the result against the query data instead of this re-running every render.
 */
const selectLastCommit = (headInfo: RefInfo): LastCommit | null => {
	let latest: LastCommit | null = null;

	for (const stack of headInfo.stacks) {
		for (const [index, segment] of stack.segments.entries()) {
			if (segment.commits.length === 0) continue;

			const named = segment.refName
				? segment
				: stack.segments.slice(index + 1).find((below) => below.refName !== null);
			const branch = named?.refName?.displayName ?? null;

			for (const commit of segment.commits) {
				if (latest === null || commit.committedAt > latest.committedAt)
					latest = { committedAt: commit.committedAt, branch };
			}
		}
	}

	return latest;
};

/**
 * The one line a clean worktree gets, under the header that says it is clean.
 *
 * It reports the last commit rather than explaining that an empty commit is
 * possible: this is the state the sidebar rests in, so the line is read
 * constantly, and "did that land, and where?" is the live question at rest
 * while an empty commit is a rare and deliberate act. A repository with no
 * commits has nothing to report, and there the empty commit is the most useful
 * thing left to say.
 *
 * Its own component so that the clock only re-renders this line, and its
 * derivation rides the query's `select` rather than running here.
 */
export const CleanWorktreeNote: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: lastCommit } = useQuery({
		...headInfoQueryOptions(projectId),
		select: selectLastCommit,
	});
	// Minutes are the finest unit this line ever shows, so a slower clock than
	// the age badges' would still read correctly; it matches theirs so the two
	// tick together rather than in two staggered renders.
	const now = useNow(lastCommit === null || lastCommit === undefined ? null : 30_000);

	if (lastCommit === undefined) return null;

	if (lastCommit === null) return <p className={styles.note}>You can still make an empty commit</p>;

	return (
		<p className={styles.note}>
			<span>Last commit {formatRelativeTime(lastCommit.committedAt, now)}</span>

			{lastCommit.branch !== null && (
				<>
					<Icon size={12} name="arrow-right" />
					<span className={styles.branch}>{lastCommit.branch}</span>
				</>
			)}
		</p>
	);
};
