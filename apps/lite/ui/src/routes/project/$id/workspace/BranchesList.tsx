import rowStyles from "./Row.module.css";
import { Scroller } from "#ui/components/Scroller.tsx";
import { useApply } from "#ui/api/mutations.ts";
import { branchDetailsQueryOptions } from "#ui/api/queries.ts";
import { encodeBytes } from "#ui/api/bytes.ts";
import { assert } from "#ui/assert.ts";
import {
	branchDetailsParams,
	branchIsEmpty,
	branchOwnCommits,
	type BranchFilters,
} from "#ui/branch.ts";
import { commitIsDiverged, commitTitle } from "#ui/commit.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { FieldControlStyles } from "#ui/components/Field.tsx";
import { GraphSegment, type GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import {
	nativeMenuItem,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import {
	branchOperand,
	commitOperand,
	operandEquals,
	operandIdentityKey,
	type Operand,
} from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useNavigationIndexHotkeys, type SelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { formatRelativeTime } from "#ui/time.ts";
import type { Commit, ListedBranch } from "@gitbutler/but-sdk";
import { Toolbar } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import {
	type ComponentProps,
	type FC,
	type MouseEvent,
	useEffect,
	useId,
	useRef,
	useState,
} from "react";
import { Row, RowLabel, RowLabelContainer, RowLabelFooter, RowToolbar } from "./Row.tsx";
import { getRowButtonClassName, treeItemId } from "./Row-utils.ts";
import type { BranchesOutline } from "./useBranchesOutline.ts";
import styles from "./BranchesList.module.css";

/** The filter menu, in the order it is shown. */
const filterMenuLabels: Array<[keyof BranchFilters, string]> = [
	["showEmpty", "Include Empty Branches"],
	["onlyLocal", "Show Only Local Branches"],
	["onlyStacks", "Show Only Stacks"],
];

/**
 * The graph has no remote-only state, so a branch that exists only on a remote
 * takes the same glyph as a synced one: it has nothing unpushed, which
 * "LocalOnly" would wrongly imply.
 */
const branchGraphStatus = (branch: ListedBranch): GraphSegmentStatus =>
	branch.remoteRefs.length > 0 ? "LocalAndRemote" : "LocalOnly";

/**
 * Whether the stored selection is this operand. Rows subscribe to this plain
 * boolean instead of consuming the navigation index, so index rebuilds (fold,
 * filter, data refresh) do not re-render every row. BranchesList keeps the
 * stored selection normalized to the resolved one.
 */
const useIsSelected = (projectId: string, operand: Operand): boolean =>
	useAppSelector((state) => {
		const stored = projectSlice.selectors.selectPrimaryBranchesSelection(state, projectId);
		return stored !== null && operandEquals(stored, operand);
	});

const InertRow: FC<{ branch: ListedBranch; label: string }> = ({ branch, label }) => (
	<Row interactive={false} role="treeitem" aria-label={label}>
		<GraphSegment glyph="parent" status={branchGraphStatus(branch)} />
		<RowLabelContainer>
			<RowLabel className={rowStyles.fadedText}>{label}</RowLabel>
		</RowLabelContainer>
	</Row>
);

const CommitItem: FC<{ projectId: string; commit: Commit }> = ({ projectId, commit }) => {
	const dispatch = useAppDispatch();
	const operand = commitOperand({ commitId: commit.id, changeId: commit.changeId });
	const isSelected = useIsSelected(projectId, operand);
	const title = commitTitle(commit.message);

	return (
		<Row
			id={treeItemId(operand)}
			role="treeitem"
			aria-label={title ?? "(no message)"}
			aria-selected={isSelected}
			isSelected={isSelected}
			onSelect={() =>
				dispatch(projectSlice.actions.selectBranches({ projectId, selection: operand }))
			}
		>
			<GraphSegment
				glyph="commit"
				status={commitIsDiverged(commit) ? "Diverged" : commit.state.type}
			/>
			<RowLabelContainer>
				<RowLabel singleLine>
					{title === undefined ? <span className={rowStyles.fadedText}>(no message)</span> : title}
				</RowLabel>
			</RowLabelContainer>
		</Row>
	);
};

const BranchCommits: FC<{ projectId: string; branch: ListedBranch }> = ({ projectId, branch }) => {
	const { data: branchDetails } = useQuery(
		branchDetailsQueryOptions({ projectId, ...branchDetailsParams(branch.refName.full) }),
	);

	if (!branchDetails) return <InertRow branch={branch} label="Loading…" />;

	const commits = branchOwnCommits(branch, branchDetails.commits);
	if (commits.length === 0) return <InertRow branch={branch} label="No commits." />;

	return commits.map((commit) => (
		<CommitItem key={commit.id} projectId={projectId} commit={commit} />
	));
};

const BranchItem: FC<{ projectId: string; branch: ListedBranch }> = ({ projectId, branch }) => {
	const dispatch = useAppDispatch();
	const branchRef = branch.refName.full;
	const operand = branchOperand({ branchRef: encodeBytes(branchRef) });
	// A branch with no commits of its own has nothing to unfold; an unknown
	// count keeps the affordance.
	const canUnfold = !branchIsEmpty(branch);
	const unfolded =
		useAppSelector((state) =>
			projectSlice.selectors.selectBranchUnfolded(state, projectId, branchRef),
		) && canUnfold;
	const isSelected = useIsSelected(projectId, operand);
	const [now] = useState(() => Date.now());

	const review = branch.review;

	const { isPending: isApplyPending, mutate: apply } = useApply();

	const toggleUnfolded = () => {
		dispatch(projectSlice.actions.toggleBranchUnfolded({ projectId, branchRef }));
	};

	const applyBranch = () => {
		apply(
			{ projectId, existingBranch: branchRef },
			{
				onSuccess: (response) => {
					const appliedRef = response.appliedBranches[0];
					if (!appliedRef) return;

					dispatch(projectSlice.actions.setOutlineTab({ projectId, tab: "workspace" }));
					dispatch(
						projectSlice.actions.selectOutline({
							projectId,
							selection: branchOperand({ branchRef: encodeBytes(appliedRef.full) }),
						}),
					);
				},
			},
		);
	};

	const openReviewInBrowser = async (evt: MouseEvent<HTMLAnchorElement>): Promise<void> => {
		evt.preventDefault();

		if (review) await window.lite.openInWebBrowser(review.htmlUrl);
	};

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: "Apply to Workspace",
			enabled: !isApplyPending,
			onSelect: applyBranch,
		}),
	];

	const lastTouched = [
		branch.lastAuthor?.name,
		branch.updatedAtMs !== null ? formatRelativeTime(branch.updatedAtMs, now) : undefined,
	]
		.filter((part) => part !== undefined)
		.join(" ");

	return (
		<div
			id={treeItemId(operand)}
			role="treeitem"
			aria-label={branch.displayName}
			aria-selected={isSelected}
			// A branch with nothing to unfold is a leaf: omit the attribute
			// entirely rather than reporting it as collapsed.
			aria-expanded={canUnfold ? unfolded : undefined}
		>
			<Row
				isSelected={isSelected}
				onSelect={() =>
					dispatch(projectSlice.actions.selectBranches({ projectId, selection: operand }))
				}
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
			>
				{canUnfold ? (
					<button
						type="button"
						aria-expanded={unfolded}
						aria-label={unfolded ? "Fold commits" : "Unfold commits"}
						className={styles.foldToggle}
						onClick={toggleUnfolded}
					>
						<GraphSegment
							glyph={unfolded ? "parent" : "group"}
							status={branchGraphStatus(branch)}
						/>
					</button>
				) : (
					<GraphSegment glyph="parent" status={branchGraphStatus(branch)} />
				)}

				<div className={styles.label}>
					<RowLabelContainer>
						<RowLabel heading singleLine title={branch.displayName}>
							{branch.displayName}
						</RowLabel>
					</RowLabelContainer>

					<RowLabelFooter className={classes("text-13", styles.labelMeta)}>
						{/* The branch's own commits, matching what unfolding reveals.
						    commitsAheadOfTarget would also count the branches below it
						    in a stack, so every row above the bottom would overstate. */}
						{branch.commitCount !== null && branch.commitCount > 0 && (
							<span className={classes(rowStyles.fadedText, styles.labelMetaItem)}>
								<Icon name="commit" />
								{branch.commitCount}
							</span>
						)}

						{lastTouched !== "" && (
							<span
								className={classes(rowStyles.fadedText, styles.labelMetaItem)}
								title={branch.lastAuthor?.email}
							>
								{lastTouched}
							</span>
						)}

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

						{isApplyPending && <Icon name="spinner" />}
					</RowLabelFooter>
				</div>

				<Toolbar.Root aria-label="Branch actions" render={<RowToolbar />}>
					<Toolbar.Button
						aria-label="Branch menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			</Row>

			{unfolded && (
				// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics.
				<div role="group">
					<BranchCommits projectId={projectId} branch={branch} />
				</div>
			)}
		</div>
	);
};

export const BranchesList: FC<
	{ projectId: string; outline: BranchesOutline } & ComponentProps<"div">
> = ({ projectId, outline, ...restProps }) => {
	const dispatch = useAppDispatch();
	// Derived once in WorkspacePage and passed down, so the rendered list and the
	// navigation index that resolves selection are the same object.
	const { stacks, navigationIndex, isPending, isError } = outline;
	const filters = useAppSelector((state) =>
		projectSlice.selectors.selectBranchFilters(state, projectId),
	);
	const search = useAppSelector((state) =>
		projectSlice.selectors.selectBranchSearch(state, projectId),
	);

	const selection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionBranches(state, projectId, navigationIndex),
	);
	const storedSelection = useAppSelector((state) =>
		projectSlice.selectors.selectPrimaryBranchesSelection(state, projectId),
	);

	// Rows highlight by comparing against the stored selection (see
	// useIsSelected), so whenever resolving against the index lands elsewhere —
	// entering the tab, or the selected item folding or filtering away — store
	// the resolved selection to keep the two in agreement.
	useEffect(() => {
		if (
			selection !== null &&
			(storedSelection === null || !operandEquals(storedSelection, selection))
		)
			dispatch(projectSlice.actions.selectBranches({ projectId, selection }));
	}, [dispatch, projectId, selection, storedSelection]);

	const headingId = useId();
	const hotkeysRef = useRef<HTMLDivElement>(null);

	useNavigationIndexHotkeys({
		navigationIndex,
		projectId,
		group: "Outline",
		select: (newItem) =>
			dispatch(projectSlice.actions.selectBranches({ projectId, selection: newItem })),
		selection,
		selectSectionPredicate: (operand) => operand._tag === "Branch",
		ref: hotkeysRef,
		getKey: operandIdentityKey,
	});

	const showFilterMenu = (trigger: HTMLElement) => {
		void showNativeMenuFromTrigger(
			trigger,
			filterMenuLabels.map(([filter, label]) =>
				nativeMenuItem({
					label,
					checked: filters[filter],
					onSelect: () => {
						dispatch(projectSlice.actions.toggleBranchFilter({ projectId, filter }));
					},
				}),
			),
		);
	};

	return (
		<div {...restProps} className={classes(restProps.className, styles.container)}>
			<div className={styles.toolbar}>
				<button
					type="button"
					aria-label="Branch filters"
					className={getButtonClassName({ iconOnly: true })}
					onClick={(evt) => showFilterMenu(evt.currentTarget)}
				>
					<Icon name="filter" />
				</button>

				<FieldControlStyles
					className={styles.filterInput}
					aria-label="Filter branches"
					placeholder="Filter branches…"
					value={search}
					onChange={(evt) =>
						dispatch(
							projectSlice.actions.setBranchSearch({ projectId, search: evt.currentTarget.value }),
						)
					}
				/>
			</div>

			<Scroller className={styles.listArea} viewportClassName={styles.list}>
				<h4 id={headingId} className={classes("text-13", styles.heading)}>
					Recent branches
				</h4>

				{stacks.length === 0 && (
					<p className={classes("text-13", styles.heading)}>
						{isPending
							? "Loading branches…"
							: isError
								? "Unable to load branches."
								: search.trim() !== ""
									? "No matching branches."
									: "No branches."}
					</p>
				)}

				<div
					tabIndex={0}
					role="tree"
					aria-labelledby={headingId}
					aria-activedescendant={selection ? treeItemId(selection) : undefined}
					data-selection-scope={"outline" satisfies SelectionScope}
					className={styles.tree}
					onFocus={() =>
						dispatch(projectSlice.actions.setDetailsSelectionScope({ projectId, scope: "outline" }))
					}
					ref={hotkeysRef}
				>
					{stacks.map((stack) => (
						// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A stack is an ARIA group of tree items.
						<div key={assert(stack.branches[0]).refName.full} role="group" className={styles.stack}>
							{stack.branches.map((branch) => (
								<BranchItem key={branch.refName.full} projectId={projectId} branch={branch} />
							))}
						</div>
					))}
				</div>
			</Scroller>
		</div>
	);
};
