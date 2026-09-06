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
import { TargetCommitRow } from "#ui/routes/project/$id/workspace/UpstreamList.tsx";
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
import {
	type CSSProperties,
	type FC,
	type ReactNode,
	type Ref,
	type RefObject,
	useRef,
} from "react";
import styles from "./Section.module.css";
import {
	CARD_X,
	LEG_BEND,
	LEG_GAP,
	MAIN_X,
	type Plan,
	rowInsetFor,
	type Run,
	targetCommitAddress,
} from "./layout.ts";

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
	railEnds = false,
) => {
	const index = addressSpace.indexByKey.get(addressIdentityKey(targetCommitAddress(commit)));
	return (
		<TargetCommitRow
			key={commit.commit.id}
			item={{ ...commit, type: "commit" }}
			list="applied"
			positionInSet={(index ?? -1) + 1}
			setSize={addressSpace.items.length}
			status={status}
			railEnds={railEnds}
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
	/** After the label, in its line: a control of the row's own. */
	glyph: ReactNode;
	className?: string;
	style?: CSSProperties;
	children?: ReactNode;
}> = ({ label, caption, heading = false, fold, glyph, className, style, children }) => (
	<Row
		interactive={fold !== undefined}
		onSelect={fold?.onToggle}
		className={className}
		style={style}
	>
		{fold === undefined ? (
			glyph
		) : (
			<RowFoldToggle
				folded={!fold.open}
				glyph={glyph}
				aria-label={`${fold.open ? "Fold" : "Unfold"} ${fold.name}`}
				onClick={fold.onToggle}
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
	onMore: () => void;
	onFold: () => void;
}> = ({ run, status, onMore, onFold }) => (
	// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A row that reveals or folds, styled as the rows around it.
	<Row role="button" onSelect={run.hidden > 0 ? onMore : onFold} aria-expanded={run.expanded}>
		<GraphSegment glyph={run.hidden > 0 ? "group" : "parent"} status={status} centered />
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
	onMore: () => void,
	onFold: () => void,
) => {
	// What is folded away is not mounted: a run can be hundreds of commits long.
	const elided = run.hidden > 0 || run.expanded;
	return (
		<div key={run.id}>
			{run.shown.map((commit) => commitRow(commit, status, addressSpace))}
			{elided && <Elided run={run} status={status} onMore={onMore} onFold={onFold} />}
		</div>
	);
};

/** Rebases every stack onto the target's fetched tip; this does not fetch. */
const Update: FC<{ projectId: string }> = ({ projectId }) => {
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
			{isPending ? "Updating…" : "Update"}
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
	/** The scroller, for the docked merge base row's toggle to scroll to its place. */
	scrollElementRef: RefObject<HTMLDivElement | null>;
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
	const chevron = (open: boolean) => (
		<Icon className={styles.chevron} name={open ? "chevron-down" : "chevron-right"} />
	);
	return (
		<>
			{plan.base !== null &&
				!plan.refOnBase &&
				(branched ? (
					// The target has moved on: a card like a forked stack's, its
					// incoming commits on a leg.
					<div
						className={styles.card}
						style={{ "--row-padding-inline-start": `${rowInsetFor(CARD_X)}px` }}
					>
						<Header
							label={plan.header.label}
							heading
							fold={{
								open: plan.incomingExpanded,
								onToggle: onToggleIncoming,
								name: "incoming commits",
							}}
							glyph={chevron(plan.incomingExpanded)}
						/>
						<Fold open={plan.incomingExpanded}>
							<div className={styles.rows}>
								{plan.incoming.map((run) =>
									runRows(
										run,
										"Upstream",
										addressSpace,
										() => onShowMoreRun(run.id),
										() => onFoldRun(run.id),
									),
								)}
							</div>
						</Fold>
						<svg className={styles.gap} aria-hidden>
							<path className={styles.main} d={`M ${MAIN_X} 0 V ${LEG_GAP}`} />
							<path className={styles.leg} d={LEG_BEND} />
						</svg>
					</div>
				) : (
					// The target sits above the base with nothing incoming: a row on
					// the main line, marked the way a branch is marked on its rail.
					<Header
						label={plan.header.label}
						heading
						glyph={<GraphSegment glyph="joinRight" status="LocalOnly" />}
						className={styles.ref}
					/>
				))}
			{plan.base !== null && (
				<>
					{/* The ref's tip on the base: one row for both. Moved on: the row says how far. */}
					<Header
						label={plan.refOnBase ? plan.header.label : "Merge base"}
						caption={
							plan.refOnBase ? (
								<span className={classes("text-12", styles.caption)}>merge base</span>
							) : branched ? (
								<span className={classes("text-12", styles.incoming)}>
									{plan.header.incoming} new
								</span>
							) : undefined
						}
						heading={plan.refOnBase}
						fold={{
							open: plan.baseExpanded,
							onToggle: toggleBase,
							name: "the merge base's history",
						}}
						glyph={chevron(plan.baseExpanded)}
						className={classes(styles.base, !plan.baseExpanded && styles.docked)}
					>
						{branched && <Update projectId={projectId} />}
					</Header>
					<Fold open={plan.baseExpanded} className={styles.history} ref={baseFold}>
						<div ref={baseRows} className={styles.rows}>
							{plan.belowBase.map((item, index) =>
								item.kind === "fork"
									? commitRow(
											item.commit,
											"Integrated",
											addressSpace,
											endsOnBase && index === plan.belowBase.length - 1,
										)
									: runRows(
											item,
											"Integrated",
											addressSpace,
											() => onShowMoreRun(item.id),
											() => onFoldRun(item.id),
										),
							)}
							{plan.older.map((commit, index) =>
								commitRow(
									commit,
									"Integrated",
									addressSpace,
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
