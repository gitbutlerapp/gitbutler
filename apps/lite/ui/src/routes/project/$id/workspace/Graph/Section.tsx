import { GraphSegment } from "#ui/components/GraphSegment.tsx";
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
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Button, Tooltip } from "@base-ui/react";
import type { BottomUpdate } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import { type FC, type ReactNode, type Ref, type RefObject, useRef } from "react";
import { createPortal } from "react-dom";
import styles from "./Section.module.css";
import { TargetCommitRow } from "./TargetCommitRow.tsx";
import { type Plan, type Run, targetCommitAddress } from "./layout.ts";

/*
 * The upstream section under the stacks: the target's row or card, folding
 * the incoming commits, then the merge base header, folding the history
 * below it. Commit rows are values on the applied cursor; headers are not.
 *
 * The trunk runs down the panel's edge and hooks into the first row here
 * that draws a glyph, the ref's or the base's, since no glyph fits on the edge.
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
	/** The fold the header opens; none for a plain row. Its chevron swaps in for the glyph on hover, unless the glyph is one. */
	fold?: { open: boolean; onToggle: () => void; name: string; hoverChevron?: boolean };
	/** The row's gutter. */
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
				glyph={rail}
				aria-label={`${fold.open ? "Fold" : "Unfold"} ${fold.name}`}
				onClick={fold.onToggle}
				hoverChevron={fold.hoverChevron}
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

const Fold: FC<{
	open: boolean;
	className?: string;
	ref?: Ref<HTMLDivElement>;
	children: ReactNode;
}> = ({ open, className, ref, children }) => (
	<div ref={ref} className={classes(styles.fold, className)} data-open={open}>
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
const Integrate: FC<{ projectId: string; target: string }> = ({ projectId, target }) => {
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
		<Tooltip.Root>
			<Tooltip.Trigger
				className={getRowButtonClassName({ variant: "outline" })}
				onClick={rebase}
				// `disabled` goes on the button so the tooltip still opens over it.
				render={<Button focusableWhenDisabled disabled={!enabled} />}
			>
				{isPending ? "Integrating…" : "Integrate"}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>
						Integrate the latest from {target} into the base
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
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
	/** The scroller, for the docked merge base row's toggle to scroll to its place. */
	scrollElementRef: RefObject<HTMLDivElement | null>;
	/** The scroller's foot, where a stand-in for the merge base row docks while the row is out of view. */
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
	scrollElementRef,
	footDock,
}) => {
	const branched = plan.header.incoming > 0;
	const addressSpace = useAddressSpace();
	// Opening from docked: scroll to the bottom and hold it while the fold grows.
	const baseFold = useRef<HTMLDivElement>(null);
	const baseRows = useRef<HTMLDivElement>(null);
	const toggleBase = () => {
		const scroller = scrollElementRef.current;
		const fold = baseFold.current;
		const rows = baseRows.current;
		if (!plan.baseExpanded && scroller !== null && fold !== null && rows !== null) {
			scroller.scrollTop = scroller.scrollHeight;
			let height = 0;
			const hold = new ResizeObserver(() => {
				scroller.scrollTop = scroller.scrollHeight;
				// Lets go once the fold has grown to its rows, or shrinks: folded again midway.
				if (fold.offsetHeight >= rows.offsetHeight || fold.offsetHeight < height) hold.disconnect();
				height = fold.offsetHeight;
			});
			hold.observe(fold);
		}
		onToggleBase();
	};
	// The line ends on the last row shown: the "show more" row, else the
	// last commit once the history is shown to its start.
	const endsOnBase = historyEnds && moreBelow === "hidden" && plan.older.length === 0;
	/**
	 * The ref's tip on the base: one row for both. The docked stand-in, with
	 * no line to show, wears the chevron instead of a glyph.
	 */
	const baseHeader = (docked = false) => (
		<Header
			label={plan.refOnBase ? plan.header.label : "Base"}
			caption={
				plan.refOnBase ? (
					<span className={classes("text-12", styles.caption)}>base</span>
				) : undefined
			}
			heading={plan.refOnBase}
			fold={{
				open: plan.baseExpanded,
				onToggle: toggleBase,
				name: "the base's history",
				hoverChevron: !docked,
			}}
			rail={
				docked ? (
					<span className={styles.chevron}>
						<Icon name={plan.baseExpanded ? "chevron-down" : "chevron-right"} />
					</span>
				) : (
					<GraphSegment
						// The trunk hooks in from the edge, meeting the target's leg coming down
						// the column, unless the ref's row above brought it into the column. No
						// mark of its own: the row is a label on the line, not a commit.
						glyph={plan.refOnBase || branched ? "hook" : "parent"}
						above={branched ? "Upstream" : undefined}
						// Folded, the hint of the history below stays in the trunk's own grey.
						below={plan.baseExpanded ? "Integrated" : undefined}
						status="LocalOnly"
						folded={!plan.baseExpanded}
					/>
				)
			}
			className={docked ? styles.docked : undefined}
		/>
	);
	return (
		<>
			{plan.base !== null &&
				!plan.refOnBase &&
				(branched ? (
					// The target has moved on: a card like a forked stack's, the trunk
					// behind its rows at the edge and its incoming commits on a leg that
					// starts at its row and runs straight down into the base's. Its row
					// says how many are new and integrates them: seeing and acting sit together.
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
								rail={<GraphSegment glyph="forkRight" status="Upstream" behind={1} />}
							>
								<Integrate projectId={projectId} target={plan.header.label} />
							</Header>
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
						<Row interactive={false} className={styles.leg}>
							<GraphSegment glyph="parent" status="Upstream" behind={1} />
						</Row>
					</>
				) : (
					// The target sits above the base with nothing incoming: the trunk
					// hooks in from the edge onto its row and runs on down the column
					// to the base.
					<>
						<Header
							label={plan.header.label}
							heading
							rail={<GraphSegment glyph="hook" status="LocalOnly" />}
						/>
						<Row interactive={false} className={styles.leg}>
							<GraphSegment glyph="parent" status="LocalOnly" />
						</Row>
					</>
				))}
			{plan.base !== null && (
				<>
					{baseHeader()}
					{/* Folded, the row's stand-in docks at the scroller's foot while the row is out
					    of view below. A portal: the foot is outside the tree, and only there can it
					    stick over the uncommitted files card, which is outside the tree as well. */}
					{!plan.baseExpanded && footDock !== null && createPortal(baseHeader(true), footDock)}
					<Fold open={plan.baseExpanded} className={styles.history} ref={baseFold}>
						<div ref={baseRows} className={styles.rows}>
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
				</>
			)}
		</>
	);
};
