import { GraphGap, GraphSegment, type GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { getRowButtonClassName } from "#ui/routes/project/$id/workspace/Row-utils.ts";
import {
	Row,
	RowFoldToggle,
	RowLabel,
	RowLabelContainer,
	RowToolbar,
} from "#ui/routes/project/$id/workspace/Row.tsx";
import { useFetchFromRemotes } from "#ui/routes/project/$id/workspace/useFetchFromRemotes.ts";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { formatRelativeTime } from "#ui/time.ts";
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
import { type FC, type ReactNode, type RefObject, useRef, useState } from "react";
import styles from "./Section.module.css";
import { TargetCommitRow } from "./TargetCommitRow.tsx";
import {
	CARD_GAP,
	HEAD_DOCKED_HEIGHT,
	LEG_GAP,
	type Plan,
	type Run,
	targetCommitAddress,
} from "./layout.ts";
import type { Graph } from "./usePlan.ts";

/*
 * The upstream section under the stacks: the target's row, standing for the
 * workspace's base, folding the incoming commits when the target has moved
 * on. Expanded, the header and incoming commits form a card. Commit rows are
 * values on the applied cursor; the header is not. History opens independently,
 * continuing from the workspace's shared target commits into older pages.
 *
 * When the target's row is below the viewport, a stand-in docks at the
 * scroller's foot to keep it in reach. A sticky mark of no height right after
 * the row determines when the stand-in appears.
 */

// The rows sit in the column beside the trunk. Rows a pending operation drops from the list are inert.
const commitRow = (
	commit: TargetCommit,
	addressSpace: AddressSpace<Address>,
	railEnds = false,
	above?: GraphSegmentStatus,
	fromTrunk = false,
) => {
	const index = addressSpace.indexByKey.get(addressIdentityKey(targetCommitAddress(commit)));
	return (
		<TargetCommitRow
			key={commit.commit.id}
			commit={commit}
			positionInSet={(index ?? -1) + 1}
			setSize={addressSpace.items.length}
			behind={commit.inWorkspace ? 0 : 1}
			railEnds={railEnds}
			above={above}
			fromTrunk={fromTrunk}
			inert={index === undefined}
		/>
	);
};

/** A note beside the header's label, explaining itself on hover. */
const Caption: FC<{ className?: string; hint: ReactNode; children: ReactNode }> = ({
	className,
	hint,
	children,
}) => (
	<Tooltip.Root>
		<Tooltip.Trigger render={<span className={classes("text-12", className)} />}>
			{children}
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Positioner sideOffset={4}>
				<Tooltip.Popup render={<TooltipPopup />}>{hint}</Tooltip.Popup>
			</Tooltip.Positioner>
		</Tooltip.Portal>
	</Tooltip.Root>
);

/** A header row: not a value. With a fold, the rail's toggle opens it, as on the rows above. */
const Header: FC<{
	label: string;
	/** Beside the label, in the label's own line: a count, or a note. */
	caption?: ReactNode;
	/** The fold the header opens; none for a plain row. */
	fold?: { open: boolean; onToggle: () => void; name: string; chevron?: "always" | "none" };
	/** The row's gutter. */
	rail: ReactNode;
	/** At the row's end, past the label. */
	toolbar?: ReactNode;
	className?: string;
	children?: ReactNode;
}> = ({ label, caption, fold, rail, toolbar, className, children }) => (
	<Row interactive={false} className={className}>
		{fold === undefined ? (
			rail
		) : (
			<RowFoldToggle
				folded={!fold.open}
				glyph={rail}
				aria-label={`${fold.open ? "Fold" : "Unfold"} ${fold.name}`}
				onClick={fold.onToggle}
				chevron={fold.chevron ?? "always"}
			/>
		)}
		<RowLabelContainer>
			<RowLabel heading singleLine>
				{label}
				{caption}
			</RowLabel>
			{children !== undefined && <span className={styles.action}>{children}</span>}
		</RowLabelContainer>
		{toolbar !== undefined && <RowToolbar forceVisible>{toolbar}</RowToolbar>}
	</Row>
);

const Elided: FC<{ run: Run; onMore: () => void; onFold: () => void }> = ({
	run,
	onMore,
	onFold,
}) => (
	<Button
		render={<Row />}
		nativeButton={false}
		onClick={run.hidden > 0 ? onMore : onFold}
		aria-expanded={run.expanded}
	>
		<GraphSegment
			glyph={run.hidden > 0 ? "group" : "parent"}
			status="Upstream"
			centered
			behind={1}
		/>
		<RowLabelContainer>
			<RowLabel singleLine className={styles.elided}>
				{run.hidden === 0 ? "Show fewer" : `${run.hidden} more`}
			</RowLabel>
		</RowLabelContainer>
	</Button>
);

const Fold: FC<{
	open: boolean;
	ref?: RefObject<HTMLDivElement | null>;
	children: ReactNode;
}> = ({ open, ref, children }) => (
	<div ref={ref} className={styles.fold} data-open={open}>
		<div className={styles.foldInner}>{children}</div>
	</div>
);

const MoreHistory: FC<{ state: Graph["historyMore"]; onMore: () => void }> = ({ state, onMore }) =>
	state === "hidden" ? null : (
		<Button
			render={<Row />}
			nativeButton={false}
			onClick={onMore}
			disabled={state === "loading"}
			focusableWhenDisabled
		>
			<GraphSegment glyph="group" status="Integrated" centered railEnds />
			<RowLabelContainer>
				<RowLabel singleLine className={styles.elided}>
					{state === "loading"
						? "Loading…"
						: state === "failed"
							? "Could not load history — retry"
							: "Show more"}
				</RowLabel>
			</RowLabelContainer>
		</Button>
	);

const runRows = (
	run: Run,
	addressSpace: AddressSpace<Address>,
	onMore: () => void,
	onFold: () => void,
) => {
	// What is folded away is not mounted: a run can be hundreds of commits long.
	const elided = run.hidden > 0 || run.expanded;
	return (
		<div key={run.id}>
			{run.shown.map((commit) => commitRow(commit, addressSpace))}
			{elided && <Elided run={run} onMore={onMore} onFold={onFold} />}
		</div>
	);
};

/** Fetches from the remotes: the target's row is what a fetch moves. */
const Fetch: FC<{ projectId: string }> = ({ projectId }) => {
	const { fetch, isPending, enabled, lastSuccessfulMs } = useFetchFromRemotes(projectId);
	const [tooltipNow, setTooltipNow] = useState(() => Date.now());
	return (
		<Tooltip.Root
			onOpenChange={(open) => {
				if (open) setTooltipNow(Date.now());
			}}
		>
			<Tooltip.Trigger
				aria-label={workspaceHotkeys.fetchFromRemotes.meta.name}
				className={getRowButtonClassName({ iconOnly: true })}
				onClick={fetch}
				// `disabled` goes on the button so the tooltip still opens over it.
				render={<Button focusableWhenDisabled disabled={!enabled} />}
			>
				<Icon name={isPending ? "spinner" : "refresh"} />
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup kbd={workspaceHotkeys.fetchFromRemotes.hotkey} />}>
						{workspaceHotkeys.fetchFromRemotes.meta.name}
						{lastSuccessfulMs != null && ` (${formatRelativeTime(lastSuccessfulMs, tooltipNow)})`}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

/** Rebases every stack onto the target's fetched tip; this does not fetch. */
const Pull: FC<{ target: string; enabled: boolean; isPending: boolean; onPull: () => void }> = ({
	target,
	enabled,
	isPending,
	onPull,
}) => (
	<Tooltip.Root>
		<Tooltip.Trigger
			className={getRowButtonClassName({ variant: "outline" })}
			onClick={onPull}
			// `disabled` goes on the button so the tooltip still opens over it.
			render={<Button focusableWhenDisabled disabled={!enabled} />}
		>
			{isPending ? "Pulling…" : "Pull latest"}
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Positioner sideOffset={4}>
				<Tooltip.Popup render={<TooltipPopup />}>
					Pull the latest from {target} into the workspace base
				</Tooltip.Popup>
			</Tooltip.Positioner>
		</Tooltip.Portal>
	</Tooltip.Root>
);

export const Section: FC<{
	projectId: string;
	plan: Plan;
	onToggleIncoming: () => void;
	onToggleHistory: () => void;
	onShowMoreHistory: () => void;
	historyMore: Graph["historyMore"];
	onShowMoreRun: (runId: string) => void;
	onFoldRun: (runId: string) => void;
	/** The scroller, for the docked row's toggle to scroll to its place. */
	scrollElementRef: RefObject<HTMLDivElement | null>;
}> = ({
	projectId,
	plan,
	onToggleIncoming,
	onToggleHistory,
	onShowMoreHistory,
	historyMore,
	onShowMoreRun,
	onFoldRun,
	scrollElementRef,
}) => {
	const addressSpace = useAddressSpace();
	// One mutation for the row and its docked stand-in: each rendering its own
	// would leave the other's button enabled while a pull runs.
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const { isPending: isPulling, mutate: integrate } = useWorkspaceIntegrateUpstream();
	const pull = () => {
		const updates = (headInfo?.stacks ?? [])
			.values()
			.map(stackBottomRelativeTo)
			.filter((relativeTo) => relativeTo != null)
			.map((relativeTo): BottomUpdate => ({ kind: "rebase", selector: relativeTo }))
			.toArray();
		integrate({ projectId, updates, dryRun: false });
	};
	// Opening from docked: scroll to the bottom and hold it while the fold grows.
	const incomingFold = useRef<HTMLDivElement>(null);
	const incomingRows = useRef<HTMLDivElement>(null);
	const toggleIncoming = () => {
		const scroller = scrollElementRef.current;
		const fold = incomingFold.current;
		const rows = incomingRows.current;
		if (!plan.incomingExpanded && scroller !== null && fold !== null && rows !== null) {
			const reveal = () => {
				const height = Math.min(fold.offsetHeight, scroller.clientHeight - HEAD_DOCKED_HEIGHT - 28);
				scroller.scrollTop += Math.max(
					0,
					fold.getBoundingClientRect().top + height - scroller.getBoundingClientRect().bottom,
				);
			};
			reveal();
			let height = 0;
			const hold = new ResizeObserver(() => {
				reveal();
				// Lets go once the fold has grown to its rows, or shrinks: folded again midway.
				if (fold.offsetHeight >= rows.offsetHeight || fold.offsetHeight < height) hold.disconnect();
				height = fold.offsetHeight;
			});
			hold.observe(fold);
		}
		onToggleIncoming();
	};
	const target = plan.header;
	if (target === null) return null;
	const branched = target.incoming > 0;
	const expanded = branched && plan.incomingExpanded;
	const continuesToHistory = expanded && plan.historyAvailable;
	/** The target's row. The docked stand-in, with no line to show, wears a chevron instead of the rail. */
	const header = (docked = false) => (
		<Header
			label={target.label}
			caption={
				branched && (
					<Caption
						className={styles.incoming}
						hint={`${target.incoming === 1 ? "A commit" : "Commits"} on ${target.label} not yet in the workspace`}
					>
						{target.incoming} new
					</Caption>
				)
			}
			fold={
				branched
					? {
							open: plan.incomingExpanded,
							onToggle: toggleIncoming,
							name: "incoming commits",
							chevron: docked ? "none" : "always",
						}
					: undefined
			}
			rail={
				docked ? (
					<span className={styles.chevron}>
						{branched && <Icon name={plan.incomingExpanded ? "chevron-down" : "chevron-right"} />}
					</span>
				) : expanded ? (
					<GraphSegment glyph="forkRight" status="Upstream" behind={1} />
				) : (
					<GraphSegment
						glyph={branched ? "notch" : "space"}
						status={branched ? "Upstream" : "LocalOnly"}
						behind={1}
						folded={!plan.historyAvailable}
					/>
				)
			}
			toolbar={<Fetch projectId={projectId} />}
			className={classes(
				styles.header,
				!expanded && styles.trunkHeader,
				docked && styles.docked,
				!docked && expanded && styles.cardHead,
			)}
		>
			{!target.current && (
				<Pull
					target={target.label}
					enabled={noOperationPending && !isPulling}
					isPending={isPulling}
					onPull={pull}
				/>
			)}
		</Header>
	);
	// The upstream leg rejoins the trunk below its card; History stays on the trunk.
	return (
		<>
			{header()}
			<div className={styles.dock}>{header(true)}</div>
			{branched && (
				<>
					<Fold open={plan.incomingExpanded} ref={incomingFold}>
						<div ref={incomingRows} className={classes(styles.rows, expanded && styles.cardBody)}>
							{plan.incoming.map((run) =>
								runRows(
									run,
									addressSpace,
									() => onShowMoreRun(run.id),
									() => onFoldRun(run.id),
								),
							)}
							{expanded && (
								<Row interactive={false} className={styles.stub}>
									<GraphSegment glyph="parent" status="Upstream" behind={1} />
								</Row>
							)}
						</div>
					</Fold>
					<GraphGap
						height={expanded ? CARD_GAP : LEG_GAP}
						bend={continuesToHistory ? "LocalOnly" : expanded ? "Upstream" : undefined}
					/>
				</>
			)}
			{plan.historyAvailable && (
				<>
					<Header
						label="History"
						className={classes(styles.header, styles.trunkHeader)}
						fold={{ open: plan.historyExpanded, onToggle: onToggleHistory, name: "history" }}
						rail={
							<GraphSegment
								glyph={plan.historyExpanded ? "space" : "hook"}
								behind={plan.historyExpanded ? 1 : 0}
								status="LocalOnly"
								railEnds={!plan.historyExpanded}
							/>
						}
					/>
					<Fold open={plan.historyExpanded}>
						{plan.history.map((commit, index) =>
							commitRow(
								commit,
								addressSpace,
								historyMore === "hidden" && index === plan.history.length - 1,
								index === 0 ? "LocalOnly" : undefined,
								index === 0,
							),
						)}
						{plan.historyExpanded && <MoreHistory state={historyMore} onMore={onShowMoreHistory} />}
					</Fold>
				</>
			)}
		</>
	);
};
