import rowStyles from "../Row.module.css";
import { setCursor } from "#ui/use-cursor.ts";
import { commitTitle } from "#ui/commit.ts";
import { classes } from "#ui/components/classes.ts";
import { GraphSegment, type GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import type { TargetCommit } from "@gitbutler/but-sdk";
import { type FC, useState } from "react";
import { Row, RowLabel, RowLabelContainer, RowLabelFooter } from "../Row.tsx";
import { treeItemId, useIsSelected } from "../Row-utils.ts";
import { targetCommitAddress } from "./layout.ts";
import styles from "./TargetCommitRow.module.css";

/** A target commit in the stacks graph's upstream section: a value on the applied cursor. */
export const TargetCommitRow: FC<{
	commit: TargetCommit;
	positionInSet: number;
	setSize: number;
	/** Base rows take the integrated colour, incoming rows the upstream's. */
	status: GraphSegmentStatus;
	/** The rail ends on this row: the history has no commit below it. */
	railEnds?: boolean;
	/** Out of its list for now, as a pending operation leaves it: not a value to move to. */
	inert?: boolean;
	/** Columns of the main line running behind the row, left of its rail. */
	behind?: number;
}> = ({ commit: targetCommit, positionInSet, setSize, status, railEnds, inert, behind }) => {
	const { commit, review } = targetCommit;
	const address = targetCommitAddress(targetCommit);
	const isSelected = useIsSelected(address, "applied");
	// A commit that landed a review is shown as that review: its title says
	// what changed, where "Merge pull request #N from …" only says that it did.
	const title = review?.title ?? commitTitle(commit.message);
	const [now] = useState(() => Date.now());

	const authorName = commit.author.name;

	return (
		<Row
			id={treeItemId(address)}
			role="treeitem"
			aria-label={title ?? "(no message)"}
			aria-level={1}
			aria-posinset={inert ? undefined : positionInSet}
			aria-setsize={inert ? undefined : setSize}
			aria-selected={isSelected}
			isSelected={isSelected}
			inert={inert}
			scrollSelectedIntoView
			onSelect={() => setCursor("applied", address)}
		>
			<GraphSegment glyph="commit" status={status} railEnds={railEnds} behind={behind} />
			<div className={styles.label}>
				<RowLabelContainer>
					<RowLabel singleLine>
						{title === undefined ? (
							<span className={rowStyles.fadedText}>(no message)</span>
						) : (
							title
						)}
					</RowLabel>
				</RowLabelContainer>
				<RowLabelFooter className={classes("text-13", styles.labelMeta)}>
					<span
						className={classes(rowStyles.fadedText, styles.labelMetaItem)}
						title={commit.author.email}
					>
						{authorName !== "" && <>{authorName} </>}
						<RelativeTime timestamp={commit.committedAt} now={now} />
					</span>

					{review !== null && (
						<span
							title={review.title}
							className={classes(rowStyles.fadedText, styles.labelMetaItem)}
						>
							<Icon name="pr" />
							{review.unitSymbol}
							{review.number}
						</span>
					)}
				</RowLabelFooter>
			</div>
		</Row>
	);
};
