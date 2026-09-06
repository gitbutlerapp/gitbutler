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
	type Plan,
	rowInsetFor,
	type Run,
	targetCommitAddress,
} from "./layout.ts";

/*
 * The upstream section under the stacks: the target's header row, folding
 * the commits incoming from it, then the base header, folding the rows below
 * the base and the older history with the "show more" row at its foot. The
 * commit rows are values on the applied list's cursor, and the Upstream
 * tab's own, so they carry review titles; the headers are not values.
 */

// Rows on the main line are the base the stacks sit on and take the integrated
// colour; rows on the incoming leg are ahead of it and take the upstream's.
// A pending operation drops them from the applied list, and then they are
// inert, as a card's rows are.
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

/**
 * A header row: not a value, so nothing selects it. With a fold, the whole
 * row toggles it; the toggle on its rail is the control assistive
 * technology sees, and other controls in the row keep their own clicks.
 */
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
	/** A rail piece of the row's own, drawn against it out of its flow. */
	rail?: ReactNode;
	className?: string;
	style?: CSSProperties;
	children?: ReactNode;
}> = ({ label, caption, heading = false, fold, glyph, rail, className, style, children }) => (
	<Row
		interactive={fold !== undefined}
		onSelect={fold?.onToggle}
		className={className}
		style={style}
	>
		{rail}
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

/**
 * Updates the workspace onto the target's tip, the sidebar's own action:
 * rebases every stack onto it. The tip is what was fetched; this does not
 * fetch.
 */
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
	/** The scroller the section is in, for the docked card's toggle to scroll to its place. */
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
	// A docked card stays folded. Its toggle scrolls to the bottom and opens
	// it, holding the bottom while the fold grows, so the rows open in view.
	const incomingFold = useRef<HTMLDivElement>(null);
	const incomingRows = useRef<HTMLDivElement>(null);
	const toggleIncoming = () => {
		const scroller = scrollElementRef.current;
		const fold = incomingFold.current;
		const rows = incomingRows.current;
		if (!plan.incomingExpanded && scroller !== null && fold !== null && rows !== null) {
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
		onToggleIncoming();
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
					// The target has moved on from the workspace's history: a card
					// like a forked stack's, a gap right of the main line, its header
					// row folding the incoming commits on their leg. Folded, the card
					// docks at the scroller's foot while its place is below it, so the
					// news stays in view.
					<div
						className={styles.card}
						data-folded={!plan.incomingExpanded}
						style={{ "--row-padding-inline-start": `${rowInsetFor(CARD_X)}px` }}
					>
						<Header
							label={plan.header.label}
							heading
							fold={{
								open: plan.incomingExpanded,
								onToggle: toggleIncoming,
								name: "incoming commits",
							}}
							glyph={chevron(plan.incomingExpanded)}
						/>
						<Fold open={plan.incomingExpanded} ref={incomingFold}>
							<div ref={incomingRows} className={styles.rows}>
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
					{/* The target's tip is the merge base itself: one row, the ref's name
					    on the base commit, so the two do not read as two commits. With
					    the target moved on, the row says how far: what Update brings. */}
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
							onToggle: onToggleBase,
							name: "the merge base's history",
						}}
						glyph={chevron(plan.baseExpanded)}
						// The leg's bend, from the target's card above onto the main line.
						rail={
							branched && (
								<svg className={styles.bend} aria-hidden>
									<path d={LEG_BEND} />
								</svg>
							)
						}
						className={styles.base}
					>
						{branched && <Update projectId={projectId} />}
					</Header>
					<Fold open={plan.baseExpanded} className={styles.history}>
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
					</Fold>
				</>
			)}
		</>
	);
};
