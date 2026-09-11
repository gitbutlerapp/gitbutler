import { setCursor } from "#ui/use-cursor.ts";
import { commitTitle } from "#ui/commit.ts";
import { GraphSegment, type GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { GRAPH_COMMIT_BEND_PADDING } from "#ui/components/graph-spacing.ts";
import type { TargetCommit } from "@gitbutler/but-sdk";
import { type FC, useId } from "react";
import { Row } from "../Row.tsx";
import { treeItemId, useIsSelected } from "../Row-utils.ts";
import { targetCommitAddress } from "./layout.ts";
import { CommitRowContent } from "../CommitRowContent.tsx";

const commitBendLabelStyle = { paddingBlockStart: GRAPH_COMMIT_BEND_PADDING };

/** A target commit in the stacks graph's upstream section: a value on the applied cursor. */
export const TargetCommitRow: FC<{
	commit: TargetCommit;
	positionInSet: number;
	setSize: number;
	/** Out of its list for now, as a pending operation leaves it: not a value to move to. */
	inert?: boolean;
	/** Columns of the main line running behind the row, left of its rail. */
	behind?: number;
	railEnds?: boolean;
	above?: GraphSegmentStatus;
	fromTrunk?: boolean;
}> = ({
	commit: targetCommit,
	positionInSet,
	setSize,
	inert,
	behind,
	railEnds,
	above,
	fromTrunk,
}) => {
	const { commit, review } = targetCommit;
	const address = targetCommitAddress(targetCommit);
	const isSelected = useIsSelected(address, "applied");
	// A commit that landed a review is shown as that review: its title says
	// what changed, where "Merge pull request #N from …" only says that it did.
	const title = review?.title ?? commitTitle(commit.message);
	const descriptionId = useId();

	return (
		<Row
			id={treeItemId(address)}
			role="treeitem"
			aria-label={title ?? "(no message)"}
			aria-describedby={descriptionId}
			aria-level={1}
			aria-posinset={inert ? undefined : positionInSet}
			aria-setsize={inert ? undefined : setSize}
			aria-selected={isSelected}
			isSelected={isSelected}
			inert={inert}
			scrollSelectedIntoView
			onSelect={() => setCursor("applied", address)}
		>
			<GraphSegment
				glyph="commit"
				status={targetCommit.inWorkspace ? "Integrated" : "Upstream"}
				above={above}
				fromTrunk={fromTrunk}
				behind={behind}
				railEnds={railEnds}
			/>
			<CommitRowContent
				commit={commit}
				title={title}
				descriptionId={descriptionId}
				style={fromTrunk ? commitBendLabelStyle : undefined}
			/>
		</Row>
	);
};
