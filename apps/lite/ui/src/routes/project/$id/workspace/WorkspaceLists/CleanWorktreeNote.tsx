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
 * What a clean worktree says under the header, set as the title's subtitle.
 *
 * Two lines: where the last commit went, which is the live question at rest —
 * did that land, and where — and that an empty commit is still on offer, said
 * here in plain sight rather than in a tooltip, since the button below has a
 * hotkey and a tooltip never reaches anyone using it. A repository with no
 * commits has nothing to report on the first line and keeps only the second.
 *
 * Its own component so that the clock only re-renders this block, and its
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

	return (
		<div className={styles.note}>
			{lastCommit !== null && (
				<p className={styles.line}>
					<span>Last commit {formatRelativeTime(lastCommit.committedAt, now)}</span>

					{lastCommit.branch !== null && (
						<>
							<Icon size={12} name="arrow-right" />
							<span className={styles.branch}>{lastCommit.branch}</span>
						</>
					)}
				</p>
			)}

			<p className={styles.line}>You can create an empty commit</p>
		</div>
	);
};
