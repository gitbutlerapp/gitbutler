import rowStyles from "./Row.module.css";
import { Scroller } from "#ui/components/Scroller.tsx";
import { commitTitle } from "#ui/commit.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import {
	GraphSegment,
	type GraphSegmentGlyph,
	type GraphSegmentStatus,
} from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { commitOperand, operandIdentityKey, type Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import {
	useAutofocusSelectionScope,
	useNavigationIndexHotkeys,
	type SelectionScope,
} from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import { Button } from "@base-ui/react";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import {
	type ComponentProps,
	type FC,
	type MouseEvent,
	useEffect,
	useId,
	useRef,
	useState,
} from "react";
import { Row, RowLabel, RowLabelContainer, RowLabelFooter } from "./Row.tsx";
import {
	selectionOutOfSync,
	treeItemId,
	useIsSelected as useIsSelectedInList,
} from "./Row-utils.ts";
import type {
	UpstreamBranchItem,
	UpstreamCommitItem,
	UpstreamListItem,
	UpstreamOutline,
} from "./useUpstreamOutline.ts";
import styles from "./UpstreamList.module.css";

const pluralRules = new Intl.PluralRules("en");

const useIsSelected = (projectId: string, operand: Operand): boolean =>
	useIsSelectedInList(projectId, operand, projectSlice.selectors.selectPrimaryUpstreamSelection);

/**
 * The target branch the incoming commits below it belong to. It heads the card
 * the way a stack's own name heads a stack card, and starts the target rail
 * the commit rows hang off.
 */
const TargetHeadRow: FC<{ label: string }> = ({ label }) => (
	// Presentational within the tree: only the commit rows are `treeitem`s, so
	// everything else the card draws is excluded from the tree's children.
	<Row role="none" interactive={false}>
		<GraphSegment glyph="forkRight" status="Upstream" />
		<RowLabelContainer>
			<RowLabel heading singleLine title={label}>
				{label}
			</RowLabel>
		</RowLabelContainer>
	</Row>
);

const TargetCommitRow: FC<{ projectId: string; item: UpstreamCommitItem }> = ({
	projectId,
	item,
}) => {
	const dispatch = useAppDispatch();
	const { commit, review, inWorkspace } = item;
	const operand = commitOperand({ commitId: commit.id, changeId: commit.changeId ?? commit.id });
	const isSelected = useIsSelected(projectId, operand);
	const title = commitTitle(commit.message);
	const [now] = useState(() => Date.now());

	const authorName = commit.author.name;

	const openReviewInBrowser = async (evt: MouseEvent<HTMLAnchorElement>): Promise<void> => {
		evt.preventDefault();

		if (review) await window.lite.openInWebBrowser(review.htmlUrl);
	};

	return (
		<Row
			id={treeItemId(operand)}
			role="treeitem"
			aria-label={title ?? "(no message)"}
			aria-selected={isSelected}
			isSelected={isSelected}
			onSelect={() =>
				dispatch(projectSlice.actions.selectUpstream({ projectId, selection: operand }))
			}
		>
			<GraphSegment glyph="commit" status={inWorkspace ? "Integrated" : "Upstream"} />
			<div className={styles.label}>
				<RowLabelContainer>
					{/* Commits the workspace already has are not dimmed: the row is
					    selectable like any other, and the rail's colour already
					    says which side of the boundary the commit is on. */}
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
						<a
							href={review.htmlUrl}
							title={review.title}
							onClick={(evt) => void openReviewInBrowser(evt)}
							className={classes(rowStyles.fadedText, styles.labelMetaItem)}
						>
							<Icon name="pr" />
							{review.unitSymbol}
							{review.number}
						</a>
					)}
				</RowLabelFooter>
			</div>
		</Row>
	);
};

/**
 * A workspace branch positioned against the target line. It carries its name
 * and nothing else — the rows exist to show where the workspace's branches
 * fork from the target, and everything actionable about a branch lives on the
 * workspace tab — except for having landed, which the rail's colour and a
 * second line both report.
 */
const UpstreamBranchRow: FC<{ item: UpstreamBranchItem; glyph: GraphSegmentGlyph }> = ({
	item,
	glyph,
}) => (
	<Row role="none" interactive={false}>
		<GraphSegment glyph={glyph} status={item.integrated ? "Integrated" : "LocalOnly"} />
		<div className={styles.label}>
			<RowLabelContainer>
				<RowLabel heading singleLine title={item.name}>
					{item.name}
				</RowLabel>
			</RowLabelContainer>

			{/* The rail's colour is easy to miss on a branch that has landed, so
			    the state is spelled out on the second line as well. */}
			{item.integrated && (
				<RowLabelFooter className={classes("text-13", styles.labelMeta)}>
					<span className={classes(rowStyles.fadedText, styles.labelMetaItem)}>integrated</span>
				</RowLabelFooter>
			)}
		</div>
	</Row>
);

/**
 * A toggle for the shared history between two fork points: target commits the
 * workspace already has, whose count measures how much older one stack's base
 * is than the one above it.
 */
const SegmentExpanderRow: FC<{
	projectId: string;
	segmentId: string;
	count: number;
	expanded: boolean;
	/** Set when the row opens the workspace rail, so nothing is drawn above it. */
	opensRail: boolean;
}> = ({ projectId, segmentId, count, expanded, opensRail }) => {
	const dispatch = useAppDispatch();

	return (
		<div role="none" className={styles.expanderRow}>
			<div className={styles.expanderRail}>
				{/* The stacked rings stand in for the commits while they are folded
				    away; once they are on screen the rail just runs past the toggle. */}
				<GraphSegment
					className={styles.expanderGlyph}
					glyph={
						opensRail ? (expanded ? "parentHead" : "groupHead") : expanded ? "parent" : "group"
					}
					status="LocalOnly"
				/>
				<GraphSegment className={styles.railStub} glyph="parent" status="LocalOnly" />
			</div>
			<Button
				aria-expanded={expanded}
				className={classes(
					// Filled while the run it reveals is on screen, so the toggle
					// reads as pressed rather than as another thing to click.
					getButtonClassName({ variant: expanded ? "gray" : "outline", size: "small" }),
					styles.expanderButton,
				)}
				title="Target commits your workspace already has, between these two fork points."
				onClick={() =>
					dispatch(projectSlice.actions.toggleUpstreamSegment({ projectId, segmentId }))
				}
			>
				{expanded
					? "hide commits"
					: `${count} commit${pluralRules.select(count) === "one" ? "" : "s"} between`}
			</Button>
		</div>
	);
};

/**
 * The prose and the button that act on the whole target section, drawn inside
 * the card against the target rail so they read as belonging to the commits
 * above them rather than to the panel.
 */
const UpdateBlock: FC<{
	incomingCount: number;
	hasIntegrated: boolean;
	canUpdate: boolean;
	isUpdatePending: boolean;
	onUpdateWorkspace: () => void;
}> = ({ incomingCount, hasIntegrated, canUpdate, isUpdatePending, onUpdateWorkspace }) => {
	const messageClassName = classes("text-12", "text-body", rowStyles.fadedText);

	return (
		<div role="none" className={styles.updateBlock}>
			<GraphSegment glyph="parent" status="Upstream" />
			<div className={styles.updateBlockBody}>
				{incomingCount > 0 ? (
					<p className={messageClassName}>
						Your workspace is {incomingCount} commit
						{pluralRules.select(incomingCount) === "one" ? "" : "s"} behind the upstream.
						<br />
						Rebase to update all stacks at once.
					</p>
				) : hasIntegrated ? (
					<p className={messageClassName}>
						Integrated branches can be cleaned up by updating the workspace.
					</p>
				) : canUpdate ? (
					<p className={messageClassName}>
						Your stacks already contain the latest upstream commits.
						<br />
						Update to advance the workspace base.
					</p>
				) : (
					<p className={messageClassName}>Your workspace is up to date.</p>
				)}
				<button
					type="button"
					className={getButtonClassName({ variant: "gray" })}
					disabled={!canUpdate}
					onClick={onUpdateWorkspace}
				>
					{isUpdatePending
						? "Updating…"
						: incomingCount > 0
							? "Rebase all stacks"
							: "Update workspace"}
				</button>
			</div>
		</div>
	);
};

/**
 * A clipped stub carrying a rail past the last row of its region, so the graph
 * runs out into the space below rather than stopping dead at the content edge.
 * The target's stub bridges the divider; the workspace's supplies the card's
 * floor.
 */
const RailTail: FC<{ status: GraphSegmentStatus; bridge?: boolean }> = ({ status, bridge }) => (
	<div aria-hidden className={classes(styles.railTail, bridge && styles.railTailBridge)}>
		<GraphSegment glyph="parent" status={status} />
	</div>
);

/**
 * The air closing an expanded shared-history run, answering the space its
 * toggle gets above it so the run reads as one block rather than as commits
 * crowding the branch below.
 */
const RunTail: FC = () => (
	<div aria-hidden className={styles.runTail}>
		<GraphSegment className={styles.railStub} glyph="parent" status="LocalOnly" />
	</div>
);

const listItem = (
	projectId: string,
	item: UpstreamListItem,
	/**
	 * Set on the region's first row, which opens the workspace rail: a branch
	 * forks it into being rather than joining it, and a fold toggle heads it
	 * with nothing drawn above the rings.
	 */
	opensRail: boolean,
) => {
	switch (item.type) {
		case "commit":
			return <TargetCommitRow key={item.commit.id} projectId={projectId} item={item} />;
		case "branch":
			return (
				<UpstreamBranchRow
					key={`${item.stackKey}/${item.name}`}
					item={item}
					glyph={opensRail ? "forkRight" : "joinRight"}
				/>
			);
		case "expander":
			return (
				<SegmentExpanderRow
					key={`expander-${item.segmentId}`}
					projectId={projectId}
					segmentId={item.segmentId}
					count={item.count}
					expanded={item.expanded}
					opensRail={opensRail}
				/>
			);
		default:
			return item satisfies never;
	}
};

export const UpstreamList: FC<
	{
		projectId: string;
		outline: UpstreamOutline;
		canUpdateWorkspace: boolean;
		isUpdatePending: boolean;
		onUpdateWorkspace: () => void;
	} & ComponentProps<"div">
> = ({
	projectId,
	outline,
	canUpdateWorkspace,
	isUpdatePending,
	onUpdateWorkspace,
	...restProps
}) => {
	const dispatch = useAppDispatch();
	// Derived once in WorkspacePage and passed down, so the rendered list and the
	// navigation index that resolves selection are the same object.
	const {
		items,
		incomingItemCount,
		targetLabel,
		incomingCount,
		hasIntegrated,
		navigationIndex,
		isPending,
		isError,
	} = outline;

	const selection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionUpstream(state, projectId, navigationIndex),
	);
	const storedSelection = useAppSelector((state) =>
		projectSlice.selectors.selectPrimaryUpstreamSelection(state, projectId),
	);

	const outOfSyncSelection = selectionOutOfSync(selection, storedSelection);
	useEffect(() => {
		if (outOfSyncSelection !== null)
			dispatch(projectSlice.actions.selectUpstream({ projectId, selection: outOfSyncSelection }));
	}, [dispatch, outOfSyncSelection, projectId]);

	const headingId = useId();
	const hotkeysRef = useRef<HTMLDivElement>(null);

	useNavigationIndexHotkeys({
		navigationIndex,
		projectId,
		group: "Outline",
		select: (newItem) =>
			dispatch(projectSlice.actions.selectUpstream({ projectId, selection: newItem })),
		selection,
		ref: hotkeysRef,
		getKey: operandIdentityKey,
	});

	// Decided by `canUpdateWorkspace`, not by whether this page has rows to
	// show: the base can be behind with nothing to list.
	const canUpdate = canUpdateWorkspace;

	const targetItems = items.slice(0, incomingItemCount);
	const workspaceItems = items.slice(incomingItemCount);
	// Whatever comes first opens the workspace rail, fold toggle or branch alike;
	// everything under it joins a rail that is already running.
	const railHead = workspaceItems[0];
	// The rail's tail carries on from the last branch, so it has to be coloured
	// the same as the segment it continues rather than always as local work.
	const lastBranch = workspaceItems.findLast((item) => item.type === "branch");

	return (
		<div {...restProps} className={classes(restProps.className, styles.container)}>
			<Scroller className={styles.listArea} viewportClassName={styles.list}>
				<h4 id={headingId} className={styles.srOnly}>
					Incoming changes{targetLabel !== null && <> from {targetLabel}</>}
				</h4>

				{/* The whole outline is one card: the target and the workspace's
				    branches are two regions of a single graph, divided rather than
				    boxed apart. */}
				<div
					tabIndex={0}
					role="tree"
					aria-labelledby={headingId}
					aria-activedescendant={selection ? treeItemId(selection) : undefined}
					data-selection-scope={"outline" satisfies SelectionScope}
					className={styles.card}
					onFocus={() =>
						dispatch(projectSlice.actions.setDetailsSelectionScope({ projectId, scope: "outline" }))
					}
					ref={useMergedRefs(hotkeysRef, useAutofocusSelectionScope())}
				>
					{targetLabel !== null && <TargetHeadRow label={targetLabel} />}

					{isError && (
						<p role="none" className={classes("text-13", styles.message)}>
							Unable to load incoming commits.
						</p>
					)}

					{!isError && isPending && items.length === 0 && (
						<p role="none" className={classes("text-13", styles.message)}>
							Loading incoming commits…
						</p>
					)}

					{!isError && !isPending && targetLabel === null && (
						<p role="none" className={classes("text-13", styles.message)}>
							No target branch is configured for this project.
						</p>
					)}

					{targetItems.map((item) => listItem(projectId, item, false))}

					{!isError && !isPending && targetLabel !== null && (
						<UpdateBlock
							incomingCount={incomingCount}
							hasIntegrated={hasIntegrated}
							canUpdate={canUpdate}
							isUpdatePending={isUpdatePending}
							onUpdateWorkspace={onUpdateWorkspace}
						/>
					)}

					{targetLabel !== null && <RailTail status="Upstream" bridge />}

					{workspaceItems.length > 0 && <div role="none" className={styles.divider} />}

					{workspaceItems.flatMap((item, index) => {
						const row = listItem(projectId, item, item === railHead);
						// Commits only appear here as the body of an expanded run, so
						// the last one before anything else is where a run closes.
						const next = workspaceItems[index + 1];
						return item.type === "commit" && next !== undefined && next.type !== "commit"
							? [row, <RunTail key={`${item.commit.id}-tail`} />]
							: [row];
					})}

					{workspaceItems.length > 0 && (
						<RailTail status={lastBranch?.integrated === true ? "Integrated" : "LocalOnly"} />
					)}
				</div>

				{outline.truncated && (
					<p className={classes("text-13", rowStyles.fadedText, styles.message)}>
						Only the most recent target history is shown.
					</p>
				)}
			</Scroller>
		</div>
	);
};
