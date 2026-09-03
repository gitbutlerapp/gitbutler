import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { useNow } from "#ui/components/useNow.ts";
import { formatCompactRelativeTime } from "#ui/time.ts";
import type { RefInfo } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import type { FC } from "react";
import styles from "./LastCommitLine.module.css";

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
 * Where the last commit went, set under "Nothing to commit" as its subtitle.
 *
 * The one line a clean worktree gets, and it reports rather than instructs:
 * this is the state the sidebar rests in, so the line is read constantly, and
 * "did that land, and where?" is the live question at rest. Written the short
 * way — the age, an arrow, the branch — because at rest it is read at a glance.
 *
 * Its own component so that the clock re-renders only this line, and its
 * derivation rides the query's `select` rather than running here.
 */
export const LastCommitLine: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: lastCommit } = useQuery({
		...headInfoQueryOptions(projectId),
		select: selectLastCommit,
	});
	// Matches the age badges' clock so the two tick together in one batched
	// render rather than as two staggered ones.
	const now = useNow(lastCommit === null || lastCommit === undefined ? null : 30_000);

	// Not loaded is not the same as nothing to report: the line holds its peace
	// on the way in rather than claiming an empty history it has not checked.
	if (lastCommit === undefined) return null;

	// A workspace with no commits of its own — an unborn HEAD, or a project just
	// added and still level with its target. Named as the workspace's emptiness
	// rather than the repository's: a clone with years of history lands here
	// too, and "No commits yet" on its own would be saying something false
	// about it.
	if (lastCommit === null)
		return <p className={classes("text-13", styles.line)}>No commits in this workspace yet</p>;

	const age = `${formatCompactRelativeTime(lastCommit.committedAt, now)} ago`;
	const spoken =
		lastCommit.branch === null
			? `Last commit ${age}`
			: `Last commit ${age} on ${lastCommit.branch}`;

	return (
		<p aria-label={spoken} className={classes("text-13", styles.line)}>
			<span className={styles.age}>Last commit {age}</span>

			{lastCommit.branch !== null && (
				<>
					<Icon size={12} name="arrow-right" />
					<span className={styles.branch}>{lastCommit.branch}</span>
				</>
			)}
		</p>
	);
};
