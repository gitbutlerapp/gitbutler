import { GraphSegment } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { classes } from "#ui/components/classes.ts";
import { getRowButtonClassName } from "#ui/routes/project/$id/workspace/Row-utils.ts";
import {
	Row,
	RowLabel,
	RowLabelContainer,
	RowToolbar,
} from "#ui/routes/project/$id/workspace/Row.tsx";
import { StackCard } from "#ui/routes/project/$id/workspace/StackCard.tsx";
import { TargetCommitRow } from "#ui/routes/project/$id/workspace/UpstreamList.tsx";
import type { TargetCommit } from "@gitbutler/but-sdk";
import { setActiveList } from "#ui/use-cursor.ts";
import { useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { Button } from "@base-ui/react";
import type { BottomUpdate } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import type { FC, Ref } from "react";
import styles from "./TrunkSection.module.css";
import { type GutterPlan, type TrunkRun, legX, rowInsetFor } from "./graph-layout.ts";

/*
 * The upstream section under the stacks: the target's header row, folding
 * the commits incoming from it, then the base header, folding the base rows
 * and the older history with the "show more" row at its foot. The rows are
 * the Upstream tab's own, so they select through the upstream cursor and
 * carry review titles.
 */

// Rows on the spine are the base the stacks sit on and take the integrated
// colour; rows on the incoming leg are ahead of it and take the upstream's.
// Positions count every commit row the section shows, in order.
const commitRow = (
	commit: TargetCommit,
	status: "Integrated" | "Upstream",
	positions: ReadonlyMap<string, number>,
	railEnds = false,
) => (
	<TargetCommitRow
		key={commit.commit.id}
		item={{ ...commit, type: "commit" }}
		positionInSet={positions.get(commit.commit.id) ?? 1}
		setSize={positions.size}
		status={status}
		railEnds={railEnds}
	/>
);

const ElidedRow: FC<{ run: TrunkRun; onToggle: () => void }> = ({ run, onToggle }) => (
	// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A row that toggles, styled as the rows around it.
	<Row role="button" onSelect={onToggle} aria-expanded={run.expanded}>
		<GraphSegment glyph={run.expanded ? "parent" : "group"} status="LocalOnly" />
		<RowLabelContainer>
			<RowLabel singleLine className={styles.elided}>
				{run.expanded
					? "Show fewer"
					: run.incoming
						? `${run.rest.length} more`
						: `${run.rest.length} ${run.rest.length === 1 ? "commit" : "commits"} already in the workspace`}
			</RowLabel>
		</RowLabelContainer>
	</Row>
);

const Fold: FC<{ open: boolean; children: React.ReactNode }> = ({ open, children }) => (
	<div className={styles.fold} data-open={open}>
		<div className={styles.foldInner}>{children}</div>
	</div>
);

const runRows = (
	run: TrunkRun,
	status: "Integrated" | "Upstream",
	positions: ReadonlyMap<string, number>,
	onToggle: () => void,
) => (
	// The folded rest is not mounted: a run can be hundreds of commits long.
	<div key={run.id}>
		{run.preview.map((commit) => commitRow(commit, status, positions))}
		{run.expanded && run.rest.map((commit) => commitRow(commit, status, positions))}
		{run.rest.length > 0 && <ElidedRow run={run} onToggle={onToggle} />}
	</div>
);

/** Lands the incoming leg: rebases every stack onto the target's tip. */
const RebaseButton: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const { isPending, mutate: integrate } = useWorkspaceIntegrateUpstream();
	const rebase = () => {
		const updates = (headInfo?.stacks ?? [])
			.values()
			.map(stackBottomRelativeTo)
			.filter((relativeTo) => relativeTo != null)
			.map((relativeTo): BottomUpdate => ({ kind: "rebase", selector: relativeTo }))
			.toArray();
		integrate({ projectId, updates, dryRun: false });
	};
	const enabled = noOperationPending && headInfo?.target?.isCurrent === false && !isPending;
	return (
		<Button
			className={getRowButtonClassName({ variant: "outline" })}
			disabled={!enabled}
			onClick={rebase}
		>
			{isPending ? "Catching up…" : "Catch up"}
		</Button>
	);
};

/** The "show more" row's state: hidden when nothing older can be asked for. */
export type MoreBelow = "hidden" | "idle" | "loading" | "failed";

/** The foot of the trunk: one click lengthens the history below the base. */
const ShowMoreRow: FC<{ state: MoreBelow; onSelect: () => void }> = ({ state, onSelect }) => (
	<Row
		// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A row that acts, styled as the rows around it.
		role="button"
		// Not selectable while loading: a second select would restart the fetch.
		onSelect={state === "loading" ? undefined : onSelect}
		interactive={state !== "loading"}
		data-graph-show-more
	>
		<GraphSegment glyph="group" status="Integrated" railEnds />
		<RowLabelContainer>
			<RowLabel singleLine className={styles.elided}>
				{state === "loading"
					? "Loading…"
					: state === "failed"
						? "Could not load older commits; try again"
						: "Show more"}
			</RowLabel>
		</RowLabelContainer>
	</Row>
);

export const TrunkSection: FC<{
	projectId: string;
	plan: GutterPlan;
	moreBelow: MoreBelow;
	/** The shown history reaches its start: the trunk ends on the last row. */
	historyEnds: boolean;
	onToggleIncoming: () => void;
	onToggleBase: () => void;
	onToggleRun: (runId: string) => void;
	onShowMore: () => void;
	ref: Ref<HTMLDivElement>;
}> = ({
	projectId,
	plan,
	moreBelow,
	historyEnds,
	onToggleIncoming,
	onToggleBase,
	onToggleRun,
	onShowMore,
	ref,
}) => {
	const branched = plan.header.incoming > 0;
	// The trunk ends on the last row shown: the "show more" row, else the
	// last commit once the history is shown to its start.
	const endsOnBase = historyEnds && moreBelow === "hidden" && plan.older.length === 0;
	const positions = new Map(
		[...plan.incoming, ...plan.trunk]
			.flatMap((item) =>
				item.kind === "fork"
					? [item.commit]
					: [...item.preview, ...(item.expanded ? item.rest : [])],
			)
			.concat(plan.older)
			.map((commit, index) => [commit.commit.id, index + 1]),
	);
	// The rows set the upstream cursor themselves; selecting one here also
	// hands Details to that list, which the Upstream tab does by being the page.
	const handOver = () => setActiveList("upstream");
	return (
		<div
			ref={ref}
			className={styles.section}
			// The rows' glyphs, and the base header's chevron, on the trunk rail.
			style={{ "--row-padding-inline-start": `${rowInsetFor(plan.railX)}px` }}
		>
			{branched ? (
				// The target has moved on from the workspace's history: its header
				// sits on the leg's line, off the trunk, and folds the incoming commits.
				<Row
					// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A row that toggles, styled as the rows around it.
					role="button"
					style={{ "--row-padding-inline-start": `${rowInsetFor(legX(plan))}px` }}
					// The button is an interactive descendant, so a click on it does not toggle.
					onSelect={onToggleIncoming}
					aria-expanded={plan.incomingExpanded}
					data-graph-header
				>
					<Icon
						className={styles.chevron}
						name={plan.incomingExpanded ? "chevron-down" : "chevron-right"}
					/>
					<RowLabelContainer>
						<RowLabel heading singleLine>
							{plan.header.label}
							<span className={classes("text-12", styles.incoming)}>
								{plan.header.incoming} incoming
							</span>
						</RowLabel>
					</RowLabelContainer>
					<RowToolbar forceVisible>
						<RebaseButton projectId={projectId} />
					</RowToolbar>
				</Row>
			) : (
				// The target sits on the trunk itself, marked the way a branch is
				// marked on its stack's rail, with nothing of its own to fold.
				<Row interactive={false} data-graph-header>
					<GraphSegment glyph="joinRight" status="LocalOnly" />
					<RowLabelContainer>
						<RowLabel heading singleLine>
							{plan.header.label}
						</RowLabel>
					</RowLabelContainer>
				</Row>
			)}
			{branched && (
				<Fold open={plan.incomingExpanded}>
					{/* oxlint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- Delegation only; the rows inside are the interactive elements. */}
					<div onClick={handOver}>
						{plan.incoming.length > 0 && (
							// A card like the stacks', its rows' glyphs on the leg's line.
							<StackCard
								className={styles.leg}
								style={{ "--row-padding-inline-start": `${rowInsetFor(legX(plan))}px` }}
								data-graph-leg-card
							>
								<div className={styles.legRows} data-graph-leg>
									{plan.incoming.map((run) =>
										runRows(run, "Upstream", positions, () => onToggleRun(run.id)),
									)}
								</div>
							</StackCard>
						)}
					</div>
				</Fold>
			)}
			{plan.base && (
				<>
					<Row
						// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A row that toggles, styled as the rows around it.
						role="button"
						className={styles.baseHeader}
						onSelect={onToggleBase}
						aria-expanded={plan.baseExpanded}
						data-graph-base-header
					>
						<Icon
							className={styles.chevron}
							name={plan.baseExpanded ? "chevron-down" : "chevron-right"}
						/>
						<RowLabelContainer>
							{/* A step under the ref's heading: the base is the ref's history, not a peer. */}
							<RowLabel singleLine className="text-bold">
								Base
								<span className={classes("text-12", styles.baseId)}>
									{plan.base.commit.id.slice(0, 7)}
								</span>
							</RowLabel>
						</RowLabelContainer>
					</Row>
					<Fold open={plan.baseExpanded}>
						{/* oxlint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- Delegation only; the rows inside are the interactive elements. */}
						<div onClick={handOver} data-graph-base-rows>
							{plan.trunk.map((item, index) =>
								item.kind === "fork"
									? commitRow(
											item.commit,
											"Integrated",
											positions,
											endsOnBase && index === plan.trunk.length - 1,
										)
									: runRows(item, "Integrated", positions, () => onToggleRun(item.id)),
							)}
							{plan.older.map((commit, index) =>
								commitRow(
									commit,
									"Integrated",
									positions,
									historyEnds && index === plan.older.length - 1,
								),
							)}
							{moreBelow !== "hidden" && <ShowMoreRow state={moreBelow} onSelect={onShowMore} />}
						</div>
					</Fold>
				</>
			)}
		</div>
	);
};
