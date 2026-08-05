import rowStyles from "./Row.module.css";
import { Scroller } from "#ui/components/Scroller.tsx";
import { useApply, useBranchRemove } from "#ui/api/mutations.ts";
import { branchDetailsQueryOptions } from "#ui/api/queries.ts";
import { decodeBytes, encodeBytes } from "#ui/api/bytes.ts";
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
import { FieldControlWithIcon, FieldRootStyles } from "#ui/components/Field.tsx";
import {
	GraphSegment,
	type GraphSegmentGlyph,
	type GraphSegmentStatus,
} from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { branchesHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { branchOperand, commitOperand, operandIdentityKey, type Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import {
	useAutofocusSelectionScope,
	useNavigationIndexHotkeys,
	type SelectionScope,
} from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import type { Commit, ListedBranch, ListedStack } from "@gitbutler/but-sdk";
import { Field, Toolbar } from "@base-ui/react";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { useQuery } from "@tanstack/react-query";
import { useHotkey } from "@tanstack/react-hotkeys";
import {
	type ComponentProps,
	type FC,
	Fragment,
	type MouseEvent,
	useEffect,
	useId,
	useRef,
	useState,
} from "react";
import {
	Row,
	RowFoldToggle,
	RowLabel,
	RowLabelContainer,
	RowLabelGroup,
	RowMeta,
	RowToolbar,
} from "./Row.tsx";
import {
	getRowButtonClassName,
	selectionOutOfSync,
	treeItemId,
	useIsSelected as useIsSelectedInList,
} from "./Row-utils.ts";
import { StackCard, StackCardHeader, StackFoldAllButton } from "./StackCard.tsx";
import stackCardStyles from "./StackCard.module.css";
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

const useIsSelected = (projectId: string, operand: Operand): boolean =>
	useIsSelectedInList(projectId, operand, projectSlice.selectors.selectPrimaryBranchesSelection);

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

const BranchItem: FC<{ projectId: string; branch: ListedBranch; isTopBranch: boolean }> = ({
	projectId,
	branch,
	isTopBranch,
}) => {
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

	// Same topology as the workspace outline: nothing above the branch means the
	// rail turns in from the right, otherwise it joins the branch above it. This
	// describes where the branch sits in the stack, so it does not change with
	// fold state.
	const railGlyph: GraphSegmentGlyph = isTopBranch ? "forkRight" : "joinRight";

	const review = branch.review;

	const { isPending: isApplyPending, mutate: apply } = useApply();
	const { isPending: isBranchRemovePending, mutate: branchRemove } = useBranchRemove();

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
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Delete Branch Reference",
			enabled: branch.hasLocal && !isBranchRemovePending,
			accelerator: toElectronAccelerator(branchesHotkeys.deleteBranchRef.hotkey),
			onSelect: () => branchRemove({ projectId, refName: encodeBytes(branchRef) }),
		}),
	];

	const lastAuthorName = branch.lastAuthor?.name;

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
					<RowFoldToggle
						folded={!unfolded}
						aria-label={unfolded ? "Fold commits" : "Unfold commits"}
						onClick={toggleUnfolded}
						glyph={<GraphSegment glyph={railGlyph} status={branchGraphStatus(branch)} />}
						foldedIndicator={<GraphSegment glyph="group" status={branchGraphStatus(branch)} />}
					/>
				) : (
					<GraphSegment glyph={railGlyph} status={branchGraphStatus(branch)} />
				)}

				<RowLabelGroup>
					<RowLabelContainer>
						<RowLabel heading singleLine title={branch.displayName}>
							{branch.displayName}
						</RowLabel>
					</RowLabelContainer>

					<RowMeta>
						{/* The branch's own commits, matching what unfolding reveals.
						    commitsAheadOfTarget would also count the branches below it
						    in a stack, so every row above the bottom would overstate.
						    Only while folded: the count stands in for the commits it
						    hides, so showing it alongside them would just be noise. */}
						{!unfolded && branch.commitCount !== null && branch.commitCount > 0 && (
							<span className={classes(rowStyles.fadedText, rowStyles.metaItem)}>
								<Icon size={14} name="commit" />
								{branch.commitCount}
							</span>
						)}

						{(lastAuthorName !== undefined || branch.updatedAtMs !== null) && (
							<span
								className={classes(rowStyles.fadedText, rowStyles.metaItem)}
								title={branch.lastAuthor?.email}
							>
								{lastAuthorName !== undefined && <>{lastAuthorName} </>}
								{branch.updatedAtMs !== null && (
									<RelativeTime timestamp={branch.updatedAtMs} now={now} />
								)}
							</span>
						)}

						{review !== null && (
							<a
								href={review.htmlUrl}
								title={review.title}
								onClick={(evt) => void openReviewInBrowser(evt)}
								className={classes(rowStyles.fadedText, rowStyles.metaItem)}
							>
								<Icon size={14} name="pr" />
								{review.unitSymbol}
								{review.number}
							</a>
						)}

						{isApplyPending && <Icon name="spinner" />}
					</RowMeta>
				</RowLabelGroup>

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

const BranchStackRow: FC<{ projectId: string; stack: ListedStack }> = ({ projectId, stack }) => {
	const dispatch = useAppDispatch();
	// A branch with no commits of its own has nothing to unfold, matching the
	// affordance on the branch rows themselves.
	const unfoldableRefs = stack.branches
		.filter((branch) => !branchIsEmpty(branch))
		.map((branch) => branch.refName.full);
	// A plain boolean, so this re-renders only when the stack crosses between
	// fully folded and not — see useIsSelected for the same reasoning.
	const anyUnfolded = useAppSelector((state) =>
		unfoldableRefs.some((branchRef) =>
			projectSlice.selectors.selectBranchUnfolded(state, projectId, branchRef),
		),
	);

	const { isPending: isApplyPending, mutate: apply } = useApply();

	const applyStack = () => {
		apply(
			// Branches run from the tip down, so applying the tip applies the
			// whole stack.
			{ projectId, existingBranch: assert(stack.branches[0]).refName.full },
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

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: "Apply Stack to Workspace",
			enabled: !isApplyPending,
			onSelect: applyStack,
		}),
	];

	return (
		<StackCardHeader
			toolbarLabel="Stack actions"
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<StackFoldAllButton
				hasMultipleBranches={stack.branches.length > 1}
				folded={!anyUnfolded}
				disabled={unfoldableRefs.length === 0}
				onToggle={() =>
					dispatch(
						projectSlice.actions.setBranchesUnfolded({
							projectId,
							branchRefs: unfoldableRefs,
							unfolded: !anyUnfolded,
						}),
					)
				}
			/>

			<Toolbar.Button
				aria-label="Stack menu"
				onClick={(event) => {
					void showNativeMenuFromTrigger(event.currentTarget, menuItems);
				}}
				className={getRowButtonClassName({ iconOnly: true })}
			>
				<Icon name="kebab" />
			</Toolbar.Button>
		</StackCardHeader>
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

	const outOfSyncSelection = selectionOutOfSync(selection, storedSelection);
	useEffect(() => {
		if (outOfSyncSelection !== null)
			dispatch(projectSlice.actions.selectBranches({ projectId, selection: outOfSyncSelection }));
	}, [dispatch, outOfSyncSelection, projectId]);

	const headingId = useId();
	const hotkeysRef = useRef<HTMLDivElement>(null);
	const { isPending: isBranchRemovePending, mutate: branchRemove } = useBranchRemove();
	const selectedBranchIsLocal =
		selection?._tag === "Branch" && decodeBytes(selection.branchRef).startsWith("refs/heads/");
	const removeBranch = (branchRef: Array<number>) => {
		branchRemove({ projectId, refName: branchRef });
	};

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

	useHotkey(
		branchesHotkeys.deleteBranchRef.hotkey,
		() => {
			if (selection?._tag === "Branch") removeBranch(selection.branchRef);
		},
		{
			enabled: selectedBranchIsLocal && !isBranchRemovePending,
			meta: branchesHotkeys.deleteBranchRef.meta,
			target: hotkeysRef,
		},
	);

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

				<Field.Root render={<FieldRootStyles />} className={styles.filterField}>
					<FieldControlWithIcon
						className="text-13"
						icon={<Icon name="search" />}
						aria-label="Filter branches"
						placeholder="Filter branches…"
						value={search}
						onChange={(evt) =>
							dispatch(
								projectSlice.actions.setBranchSearch({
									projectId,
									search: evt.currentTarget.value,
								}),
							)
						}
					/>
				</Field.Root>
			</div>

			<Scroller className={styles.listArea} viewportClassName={styles.list}>
				<h4 id={headingId} className={classes("text-13", "text-bold", styles.heading)}>
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
					ref={useMergedRefs(hotkeysRef, useAutofocusSelectionScope())}
				>
					{stacks.map((stack) => (
						<StackCard
							key={assert(stack.branches[0]).refName.full}
							// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A stack is an ARIA group of tree items.
							role="group"
							aria-label="Stack"
							header={<BranchStackRow projectId={projectId} stack={stack} />}
							bodyClassName={styles.stackBody}
						>
							{stack.branches.map((branch, index) => (
								<Fragment key={branch.refName.full}>
									<BranchItem projectId={projectId} branch={branch} isTopBranch={index === 0} />

									{/* Carries the rail down to the next branch, and past the
									    last one as the card's floor — as the workspace card's
									    segment connectors do. */}
									<Row interactive={false} className={stackCardStyles.railConnector}>
										<GraphSegment glyph="parent" status={branchGraphStatus(branch)} />
									</Row>
								</Fragment>
							))}
						</StackCard>
					))}
				</div>
			</Scroller>
		</div>
	);
};
