import rowStyles from "../Row.module.css";
import { decodeBytes } from "#ui/api/bytes.ts";
import { branchAddress, commitAddress, addressIdentityKey } from "#ui/addresses.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { openUpdateFromRemote } from "./update-from-remote.ts";
import { integrationPlanQueryOptions } from "#ui/branch-integration.ts";
import { authorTooltip, commitTitle } from "#ui/commit.ts";
import { GraphSegment, type GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { classes } from "#ui/components/classes.ts";
import { getRowButtonClassName } from "../Row-utils.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { isRewrittenOnly } from "#ui/segment.ts";
import { addressSpaceIncludes } from "#ui/workspace/address-space.ts";
import type { BranchReference, Segment } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import type { FC } from "react";
import { Row, RowFoldToggle, RowLabel, RowLabelContainer } from "../Row.tsx";
import { useAddressSpace } from "./context.tsx";
import styles from "./RemoteLeg.module.css";

/**
 * One commit on the leg's rail, ghosted, dragging as a cherry-pick: the
 * workspace does not hold these commits, so dropping one on a branch or
 * commit copies it through the ordinary transfer machinery.
 */
const LegCommitRow: FC<{
	projectId: string;
	inert: boolean;
	graphStatus: GraphSegmentStatus;
	commitId: string;
	changeId: string | null;
	subject: string | undefined;
	tooltip: string;
}> = ({ projectId, inert, graphStatus, commitId, changeId, subject, tooltip }) => (
	<OperationSourceC
		projectId={projectId}
		sources={[commitAddress({ commitId, changeId: changeId ?? commitId })]}
		respectChecked={false}
		kind="copy"
		outline="outside"
		render={<Row interactive={false} inert={inert} title={tooltip} />}
	>
		<div className={styles.rails}>
			<GraphSegment glyph="parent" status={graphStatus} />
			<GraphSegment glyph="commit" status="Upstream" />
		</div>
		<RowLabelContainer>
			<RowLabel singleLine className={rowStyles.fadedText}>
				{subject ?? "(no message)"}
			</RowLabel>
		</RowLabelContainer>
	</OperationSourceC>
);

/** A rail-borne message row: loading, or the fetch that failed. */
const LegNoticeRow: FC<{ graphStatus: GraphSegmentStatus; children: string }> = ({
	graphStatus,
	children,
}) => (
	<Row interactive={false}>
		<div className={styles.rails}>
			<GraphSegment glyph="parent" status={graphStatus} />
			<GraphSegment glyph="parent" status="Upstream" />
		</div>
		<RowLabelContainer>
			<RowLabel singleLine className={rowStyles.fadedText}>
				{children}
			</RowLabel>
		</RowLabelContainer>
	</Row>
);

/**
 * The branch's remote counterpart, drawn as a second rail beside the branch's
 * own: `origin/<branch>` and the commits it has that the local branch does
 * not. Display only — until now the remote's side of a divergence was
 * invisible, with force push the only offer; resolving it is the update flow's
 * job.
 *
 * Collapsed to its summary row by default; expanding reveals the remote-only
 * commits. Rewritten history — the amend case — prunes every remote commit
 * that has a similar local counterpart out of head info, so a leg with
 * nothing to list fetches the remote's versions from the integration plan's
 * divergence display instead, only while expanded.
 *
 * The rows are not in the address space — nothing here can be selected — so
 * they dim with the branch they belong to, like the segment's other
 * addressless rows.
 */
export const RemoteLeg: FC<{
	projectId: string;
	segment: Segment;
	refName: BranchReference;
	/** The colour of the branch's own rail, which the leg runs beside. */
	graphStatus: GraphSegmentStatus;
}> = ({ projectId, segment, refName, graphStatus }) => {
	const dispatch = useAppDispatch();
	const addressSpace = useAddressSpace();
	const branchRef = decodeBytes(refName.fullNameBytes);
	// Plain booleans, so this re-renders only on this branch's own state.
	const expanded = useAppSelector((state) =>
		projectSlice.selectors.selectRemoteLegExpanded(state, projectId, branchRef),
	);
	const isFolded = useAppSelector((state) =>
		projectSlice.selectors.selectSegmentFolded(state, projectId, branchRef),
	);

	const commits = segment.commitsOnRemote;
	// Force-only divergence — rewritten history whose remote counterparts are
	// pruned as similar — has nothing in head info to list.
	const hasCommits = commits.length > 0;
	// Catching up rewrites every commit, so afterwards every pushed branch is
	// "diverged" in a way that only ever wants a force push. That leg keeps
	// out of the way while folded: no rings, no button, one faded word —
	// and opens into the full leg for the rare case that is not what it looks
	// like, such as the same commit amended on both sides.
	const quiet = isRewrittenOnly(segment) && !expanded;
	// What the remote holds where head info holds nothing: the plan's
	// divergence display keeps the rewritten commits' remote versions.
	// Subscribed only while the empty leg is open, and dropped again with it.
	const {
		data: remoteVersions,
		isPending: isRemoteVersionsPending,
		isError: isRemoteVersionsError,
	} = useQuery({
		...integrationPlanQueryOptions({ projectId, branch: branchRef, strategy: "pullRebase" }),
		enabled: expanded && !hasCommits && !isFolded,
		select: (plan) => plan.divergence.upstreamOnly,
	});

	const remoteRef = segment.remoteTrackingRefName;
	// Folding hides the segment's commits; the leg goes with them. The parent
	// gates on canIntegrateUpstream, which requires the remote ref, so the
	// null check only narrows the type.
	if (remoteRef === null || isFolded) return null;

	const inert = !addressSpaceIncludes(
		addressSpace,
		branchAddress({ branchRef: refName.fullNameBytes }),
		addressIdentityKey,
	);

	const remoteLabel = `${remoteRef.remoteName}/${remoteRef.displayName}`;
	const toggle = () =>
		dispatch(projectSlice.actions.toggleRemoteLegExpanded({ projectId, branchRef }));

	return (
		<div>
			{/* Air between the branch row and the leg's head, with the branch's
			    own rail drawn through it so the line does not break. */}
			<div aria-hidden className={styles.legAir}>
				<GraphSegment glyph="parent" status={graphStatus} />
			</div>

			<Row
				interactive={false}
				inert={inert}
				onSelect={toggle}
				className={quiet ? styles.quiet : undefined}
				title={
					quiet
						? `Your commits were rewritten; ${remoteLabel} holds the previous versions. Push updates it.`
						: undefined
				}
			>
				<div className={styles.rails}>
					<GraphSegment glyph="parent" status={graphStatus} />
					<RowFoldToggle
						folded={!expanded}
						aria-label={expanded ? `Collapse ${remoteLabel}` : `Expand ${remoteLabel}`}
						onClick={toggle}
						glyph={
							// Open, the tip's bubble heads the leg and the track starts
							// below it — nothing runs on above the leg's own head. Quiet,
							// a bare stub stands in for rings that would promise commits.
							<span className={styles.remoteRail}>
								<GraphSegment
									glyph={expanded ? "commitHead" : quiet ? "parentHead" : "groupHead"}
									status="Upstream"
								/>
								<GraphSegment className={styles.railStub} glyph="parent" status="Upstream" />
							</span>
						}
					/>
				</div>
				<RowLabelContainer>
					<RowLabel singleLine title={remoteLabel} className={rowStyles.fadedText}>
						{remoteLabel}
					</RowLabel>
					{/* Only while the commits are hidden: the label stands in for
					    what it cannot show. */}
					{!expanded && (
						<span className={classes("text-13", rowStyles.fadedText, styles.ahead)}>
							{hasCommits ? `${commits.length} ahead` : quiet ? "rewritten" : "diverged"}
						</span>
					)}

					{/* In the label flow rather than at the row's far edge, and
					    ghosted: the branch row's Push next door is the primary
					    action, and this must not outshout it. Quiet, that Push is
					    the whole answer, so the button waits behind the chevron. */}
					{!quiet && (
						<button
							type="button"
							aria-label={`Integrate ${remoteLabel} into ${refName.displayName}`}
							className={classes(getRowButtonClassName({ variant: "ghost" }), rowStyles.metaButton)}
							onClick={() => openUpdateFromRemote(dispatch, refName.fullNameBytes)}
						>
							Integrate
							<Icon size={12} name="arrow-down" />
						</button>
					)}
				</RowLabelContainer>
			</Row>

			{expanded &&
				(hasCommits ? (
					commits.map((commit) => (
						<LegCommitRow
							key={commit.id}
							projectId={projectId}
							inert={inert}
							graphStatus={graphStatus}
							commitId={commit.id}
							changeId={commit.changeId}
							subject={commitTitle(commit.message)}
							tooltip={authorTooltip(commit.author, commit.committedAt)}
						/>
					))
				) : isRemoteVersionsError ? (
					<LegNoticeRow graphStatus={graphStatus}>
						Could not load the remote's commits.
					</LegNoticeRow>
				) : isRemoteVersionsPending ? (
					<LegNoticeRow graphStatus={graphStatus}>Loading the remote's commits…</LegNoticeRow>
				) : remoteVersions.length === 0 ? (
					<LegNoticeRow graphStatus={graphStatus}>
						The remote holds nothing the branch lacks.
					</LegNoticeRow>
				) : (
					remoteVersions.map((commit) => (
						<LegCommitRow
							key={commit.id}
							projectId={projectId}
							inert={inert}
							graphStatus={graphStatus}
							commitId={commit.id}
							changeId={commit.changeId}
							subject={commit.subject === "" ? undefined : commit.subject}
							tooltip={authorTooltip(commit.author, commit.createdAt)}
						/>
					))
				))}

			{/* Returns the leg into the branch's rail. The shared history the leg
			    actually forks from sits below what the outline shows, so the join
			    stands in for it the way the card's floor stands in for the base. */}
			<Row interactive={false} inert={inert} className={styles.connector}>
				<div className={styles.rails}>
					<GraphSegment glyph="joinRight" status={graphStatus} />
					<GraphSegment glyph="mergeLeft" status="Upstream" />
				</div>
			</Row>
		</div>
	);
};
