import { GraphGap, GraphSegment } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { classes } from "#ui/components/classes.ts";
import { getRowButtonClassName } from "#ui/routes/project/$id/workspace/Row-utils.ts";
import {
	Row,
	RowFoldToggle,
	RowLabel,
	RowLabelContainer,
} from "#ui/routes/project/$id/workspace/Row.tsx";
import { useAddressSpace } from "#ui/routes/project/$id/workspace/WorkspaceLists/context.tsx";
import { addressIdentityKey, type Address } from "#ui/addresses.ts";
import type { AddressSpace } from "#ui/workspace/address-space.ts";
import type { TargetCommit } from "@gitbutler/but-sdk";
import { useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { Button } from "@base-ui/react";
import type { BottomUpdate } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import type { FC, ReactNode } from "react";
import { createPortal } from "react-dom";
import styles from "./Section.module.css";
import { TargetCommitRow } from "./TargetCommitRow.tsx";
import { HISTORY_GAP, LEG_GAP, type Plan, type Run, targetCommitAddress } from "./layout.ts";

/*
 * The upstream section under the stacks: the target's row or card, folding
 * the incoming commits, then the merge base header, folding the history
 * below it. Commit rows are values on the applied cursor; headers are not.
 */

// Base rows take the integrated colour, incoming rows the upstream's. Rows a
// pending operation drops from the list are inert.
const commitRow = (
	commit: TargetCommit,
	status: "Integrated" | "Upstream",
	addressSpace: AddressSpace<Address>,
	behind: number,
	railEnds = false,
) => {
	const index = addressSpace.indexByKey.get(addressIdentityKey(targetCommitAddress(commit)));
	return (
		<TargetCommitRow
			key={commit.commit.id}
			commit={commit}
			positionInSet={(index ?? -1) + 1}
			setSize={addressSpace.items.length}
			status={status}
			railEnds={railEnds}
			behind={behind}
			inert={index === undefined}
		/>
	);
};

/** A header row: not a value. With a fold, the whole row toggles it; the rail toggle is the control assistive technology sees. */
const Header: FC<{
	label: string;
	/** Beside the label, in the label's own line: a count, an id. */
	caption?: ReactNode;
	/** The ref's row reads as a heading; the base's a step under it, being the ref's history. */
	heading?: boolean;
	/** The fold the header opens; none for a plain row. */
	fold?: { open: boolean; onToggle: () => void; name: string };
	/** The row's gutter; with a fold, its chevron sits on the glyph. */
	rail: ReactNode;
	className?: string;
	children?: ReactNode;
}> = ({ label, caption, heading = false, fold, rail, className, children }) => (
	<Row interactive={fold !== undefined} onSelect={fold?.onToggle} className={className}>
		{fold === undefined ? (
			rail
		) : (
			<RowFoldToggle
				folded={!fold.open}
				glyph={
					<span className={styles.control}>
						{rail}
						<span className={styles.chevron}>
							<Icon name={fold.open ? "chevron-down" : "chevron-right"} />
						</span>
					</span>
				}
				aria-label={`${fold.open ? "Fold" : "Unfold"} ${fold.name}`}
				onClick={fold.onToggle}
				hoverChevron={false}
			/>
		)}
		<RowLabelContainer>
			<RowLabel heading={heading} singleLine className={heading ? undefined : "text-bold"}>
				{label}
				{caption}
			</RowLabel>
			{children !== undefined && <span className={styles.action}>{children}</span>}
		</RowLabelContainer>
	</Row>
);

const Elided: FC<{
	run: Run;
	status: "Integrated" | "Upstream";
	behind: number;
	onMore: () => void;
	onFold: () => void;
}> = ({ run, status, behind, onMore, onFold }) => (
	// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A row that reveals or folds, styled as the rows around it.
	<Row role="button" onSelect={run.hidden > 0 ? onMore : onFold} aria-expanded={run.expanded}>
		<GraphSegment
			glyph={run.hidden > 0 ? "group" : "parent"}
			status={status}
			centered
			behind={behind}
		/>
		<RowLabelContainer>
			<RowLabel singleLine className={styles.elided}>
				{run.hidden === 0
					? "Show fewer"
					: run.incoming
						? `${run.hidden} more`
						: `${run.hidden} ${run.hidden === 1 ? "commit" : "commits"} already in the workspace`}
			</RowLabel>
		</RowLabelContainer>
	</Row>
);

const Fold: FC<{ open: boolean; className?: string; children: ReactNode }> = ({
	open,
	className,
	children,
}) => (
	<div className={classes(styles.fold, className)} data-open={open}>
		<div className={styles.foldInner}>{children}</div>
	</div>
);

const runRows = (
	run: Run,
	status: "Integrated" | "Upstream",
	addressSpace: AddressSpace<Address>,
	behind: number,
	onMore: () => void,
	onFold: () => void,
) => {
	// What is folded away is not mounted: a run can be hundreds of commits long.
	const elided = run.hidden > 0 || run.expanded;
	return (
		<div key={run.id}>
			{run.shown.map((commit) => commitRow(commit, status, addressSpace, behind))}
			{elided && (
				<Elided run={run} status={status} behind={behind} onMore={onMore} onFold={onFold} />
			)}
		</div>
	);
};

/** Rebases every stack onto the target's fetched tip; this does not fetch. */
const Update: FC<{ projectId: string; incoming?: number }> = ({ projectId, incoming }) => {
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
			{isPending ? "Updating…" : incoming === undefined ? "Update" : `Update (${incoming} new)`}
		</Button>
	);
};

/** The "show more" row's state: hidden when nothing older can be asked for. */
export type MoreBelow = "hidden" | "idle" | "loading" | "failed";

/** The foot of the section: one click lengthens the history below the base. */
const ShowMore: FC<{ state: MoreBelow; onSelect: () => void }> = ({ state, onSelect }) => (
	<Row
		// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A row that acts, styled as the rows around it.
		role="button"
		// Not selectable while loading: a second select would restart the fetch.
		onSelect={state === "loading" ? undefined : onSelect}
		interactive={state !== "loading"}
	>
		<GraphSegment glyph="group" status="Integrated" railEnds centered />
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

export const Section: FC<{
	projectId: string;
	plan: Plan;
	moreBelow: MoreBelow;
	/** The shown history reaches its start: the line ends on the last row. */
	historyEnds: boolean;
	onToggleIncoming: () => void;
	onToggleBase: () => void;
	onShowMoreRun: (runId: string) => void;
	onFoldRun: (runId: string) => void;
	onShowMore: () => void;
	/** The scroller's foot, where a stand-in for the base row docks while the row is out of view. */
	footDock: HTMLDivElement | null;
}> = ({
	projectId,
	plan,
	moreBelow,
	historyEnds,
	onToggleIncoming,
	onToggleBase,
	onShowMoreRun,
	onFoldRun,
	onShowMore,
	footDock,
}) => {
	const branched = plan.header.incoming > 0;
	const addressSpace = useAddressSpace();
	// The line ends on the last row shown: the "show more" row, else the
	// last commit once the history is shown to its start.
	const endsOnBase = historyEnds && moreBelow === "hidden" && plan.older.length === 0;
	/** The base's row. Docked, Update names the count. */
	const baseRow = (className?: string, incoming?: number) => (
		<Header
			label="Workspace base"
			rail={<GraphSegment glyph="commit" status="Integrated" />}
			className={className}
		>
			<Update projectId={projectId} incoming={incoming} />
		</Header>
	);
	const history = (
		<Fold open={plan.baseExpanded} className={styles.history}>
			<div className={styles.rows}>
				{plan.belowBase.map((item, index) =>
					item.kind === "fork"
						? commitRow(
								item.commit,
								"Integrated",
								addressSpace,
								0,
								endsOnBase && index === plan.belowBase.length - 1,
							)
						: runRows(
								item,
								"Integrated",
								addressSpace,
								0,
								() => onShowMoreRun(item.id),
								() => onFoldRun(item.id),
							),
				)}
				{plan.older.map((commit, index) =>
					commitRow(
						commit,
						"Integrated",
						addressSpace,
						0,
						historyEnds && index === plan.older.length - 1,
					),
				)}
				{moreBelow !== "hidden" && <ShowMore state={moreBelow} onSelect={onShowMore} />}
			</div>
		</Fold>
	);
	return (
		<>
			{plan.base !== null && branched && (
				// The target has moved on: a card like a forked stack's, the main
				// line behind its rows and its incoming commits on a leg that
				// starts under the chevron and bends onto the line in the gap below.
				// Up to date, the target is the base and needs no row of its own.
				<>
					<div className={styles.card}>
						<Row interactive={false} className={styles.air}>
							<GraphSegment glyph="space" status="LocalOnly" behind={1} />
						</Row>
						<Header
							label={plan.header.label}
							caption={
								<span className={classes("text-12", styles.incoming)}>
									{plan.header.incoming} new
								</span>
							}
							heading
							fold={{
								open: plan.incomingExpanded,
								onToggle: onToggleIncoming,
								name: "incoming commits",
							}}
							rail={<GraphSegment glyph="controlHead" status="Upstream" behind={1} />}
						/>
						<Fold open={plan.incomingExpanded}>
							<div className={styles.rows}>
								{plan.incoming.map((run) =>
									runRows(
										run,
										"Upstream",
										addressSpace,
										1,
										() => onShowMoreRun(run.id),
										() => onFoldRun(run.id),
									),
								)}
							</div>
						</Fold>
						<Row interactive={false} className={styles.stub}>
							<GraphSegment glyph="parent" status="Upstream" behind={1} />
						</Row>
					</div>
					<GraphGap height={LEG_GAP} bend="Upstream" />
				</>
			)}
			{plan.base !== null && (
				// The base's history folds inside a card like a stack's.
				<>
					{/* Moved on, the base gets a row; folded, its stand-in docks at the scroller's
					    foot while the row is out of view below. A portal: the foot is outside the
					    tree, and only there can it stick over the uncommitted files card, which is
					    outside the tree as well. Up to date, the history card alone marks the base. */}
					{branched && (
						<>
							{baseRow()}
							{!plan.baseExpanded &&
								footDock !== null &&
								createPortal(baseRow(styles.docked, plan.header.incoming), footDock)}
							<GraphGap height={HISTORY_GAP} />
						</>
					)}
					<div className={styles.historyCard}>
						<Header
							label="History"
							fold={{
								open: plan.baseExpanded,
								onToggle: onToggleBase,
								name: "the workspace base's history",
							}}
							rail={
								<GraphSegment glyph="control" status="Integrated" railEnds={!plan.baseExpanded} />
							}
						/>
						{history}
					</div>
				</>
			)}
		</>
	);
};
