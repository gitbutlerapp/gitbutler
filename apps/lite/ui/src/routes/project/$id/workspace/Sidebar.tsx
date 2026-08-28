import { useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import { setPage, usePage } from "#ui/use-cursor.ts";
import {
	forgeInfoOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	listReviewsQueryOptions,
	workspaceFetchQueryOptions,
	workspaceFetchStatusQueryOptions,
} from "#ui/api/queries.ts";
import { NotificationBell } from "#ui/review-inbox-bell.tsx";
import { usePrNotificationsLevel, useUnreadReviewCount } from "#ui/review-seen.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import type { Address } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import type { AddressSpace } from "#ui/workspace/address-space.ts";
import { Button, Toast, Toggle, ToggleGroup, Tooltip } from "@base-ui/react";
import type { BottomUpdate, ProjectForFrontend } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { type FC, useRef } from "react";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { WorkspaceLists } from "#ui/routes/project/$id/workspace/WorkspaceLists/WorkspaceLists.tsx";
import { BranchesList } from "#ui/routes/project/$id/workspace/BranchesList.tsx";
import type { BranchesListData } from "#ui/routes/project/$id/workspace/useBranchesList.ts";
import { UpstreamList } from "#ui/routes/project/$id/workspace/UpstreamList.tsx";
import type { UpstreamListData } from "#ui/routes/project/$id/workspace/useUpstreamList.ts";
import { assert } from "#ui/assert.ts";
import { Badge } from "#ui/components/Badge.tsx";
import type { PageId } from "#ui/projects/project.ts";
import styles from "./Sidebar.module.css";
import { SidebarHeader } from "#ui/routes/project/$id/workspace/SidebarHeader.tsx";
import { useNewBranch } from "#ui/routes/project/$id/workspace/useNewBranch.ts";
import { showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { RowToolbar } from "#ui/routes/project/$id/workspace/Row.tsx";
import { getRowButtonClassName } from "#ui/routes/project/$id/workspace/Row-utils.ts";

/** The tabs in the order they are shown, for cycling with `[` and `]`. */
const pageOrder: Array<PageId> = ["workspace", "upstream", "branches"];

const adjacentPage = (tab: PageId, offset: -1 | 1): PageId => {
	const index = pageOrder.indexOf(tab);
	return assert(pageOrder[(index + offset + pageOrder.length) % pageOrder.length]);
};

/**
 * Counts past this are shown as `99+`: the badge sits inside a tab, where a
 * third digit takes width from the tab labels, and at that size the number is
 * a rough sense of how far behind the workspace is rather than a figure to
 * read. The upstream page states the exact count.
 */
const maxBadgeCount = 99;

/**
 * How many applied-branch pull requests have unread activity. Its own
 * component so its subscriptions wake only this badge, not the sidebar.
 */
const WorkspaceActivityBadge: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const notificationsLevel = usePrNotificationsLevel();
	const prService = !!forgeInfo?.capabilities.prService && notificationsLevel !== "off";
	const { data: appliedBranches } = useQuery({
		...headInfoQueryOptions(projectId),
		enabled: prService,
		select: (headInfo) =>
			new Set(
				headInfo.stacks.flatMap((stack) =>
					stack.segments.flatMap((segment) => segment.refName?.displayName ?? []),
				),
			),
	});
	const { data: appliedReviews } = useQuery({
		...listReviewsQueryOptions({ projectId, cacheConfig: "noCache" }),
		enabled: prService,
		select: (reviews) =>
			reviews
				.filter((review) => appliedBranches?.has(review.sourceBranch) === true)
				.map((review) => ({ number: review.number, modifiedAt: review.modifiedAt })),
	});
	const count = useUnreadReviewCount(projectId, appliedReviews ?? [], prService);
	if (count === 0) return null;

	return <Badge variant="fillGray">{count > maxBadgeCount ? `${maxBadgeCount}+` : count}</Badge>;
};

export const Sidebar: FC<{
	absorptionTargetCommitIds: ReadonlySet<string>;
	branchesList: BranchesListData;
	upstreamList: UpstreamListData;
	addressSpace: AddressSpace<Address>;
	uncommittedAddressSpace: AddressSpace<string>;
	onActiveFileSelection: (selection: string) => void;
	project: ProjectForFrontend;
	projectId: string;
}> = ({
	absorptionTargetCommitIds,
	branchesList,
	upstreamList,
	addressSpace,
	uncommittedAddressSpace,
	onActiveFileSelection,
	project,
	projectId,
}) => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const page = usePage();

	const selectPage = (value: Array<PageId>) => {
		const head = value[0];
		if (head === undefined) return;

		setPage(head);
	};

	const openApplyBranchPicker = () => {
		dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "ApplyBranchPicker" } }));
	};

	const openProjectPicker = () => {
		dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "ProjectPicker" } }));
	};

	const openSettings = () => {
		dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "Settings" } }));
	};

	const newBranch = useNewBranch(projectId);

	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const { data: autoFetchFrequency } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.autoFetchFrequency,
	});
	const { data: workspaceFetchStatus } = useQuery(workspaceFetchStatusQueryOptions(projectId));
	const { isPending: isWorkspaceIntegrateUpstreamPending, mutate: workspaceIntegrateUpstream } =
		useWorkspaceIntegrateUpstream();
	const { isFetching: isWorkspaceFetchFromRemotesPending, refetch: workspaceFetchFromRemotes } =
		useQuery(workspaceFetchQueryOptions(projectId, autoFetchFrequency));
	const fetchFromRemotes = () => {
		void workspaceFetchFromRemotes().then(({ error }) => {
			if (!error) return;

			// oxlint-disable-next-line no-console
			console.error(error);
			toastManager.add({
				type: "error",
				title: "Failed to fetch",
				description: errorMessageForToast(error),
				priority: "high",
			});
		});
	};
	const updateWorkspace = () => {
		const rebaseUpdates = (headInfo?.stacks ?? [])
			.values()
			.map(stackBottomRelativeTo)
			.filter((relativeTo) => relativeTo != null)
			.map((relativeTo): BottomUpdate => ({ kind: "rebase", selector: relativeTo }));

		workspaceIntegrateUpstream({ projectId, updates: rebaseUpdates.toArray(), dryRun: false });
	};

	// Only an update advances the stored target, so there is work to do exactly
	// while it trails the target ref. Counting upstream commits misses the case
	// where a lane already contains them.
	const canUpdateWorkspace =
		noOperationPending &&
		headInfo?.target?.isCurrent === false &&
		!isWorkspaceIntegrateUpstreamPending;
	const canFetchFromRemotes = noOperationPending && !isWorkspaceFetchFromRemotesPending;

	const canCreateBranch = newBranch.enabled;

	const ref = useRef<HTMLDivElement>(null);

	useHotkeys([
		{
			hotkey: workspaceHotkeys.applyBranch.hotkey,
			callback: openApplyBranchPicker,
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.applyBranch.meta,
				enabled: noOperationPending,
			},
		},
		{
			hotkey: workspaceHotkeys.createIndependentBranch.hotkey,
			callback: newBranch.createInWorkspace,
			options: {
				conflictBehavior: "allow",
				enabled: canCreateBranch,
				meta: workspaceHotkeys.createIndependentBranch.meta,
				requireReset: true,
			},
		},
		// Bound beside its unshifted twin rather than on the branches tab that
		// first offered it: both `+` buttons now put it on their menu, so the
		// accelerator they show has to work wherever the sidebar is.
		{
			hotkey: workspaceHotkeys.createBranchAndSwitch.hotkey,
			callback: newBranch.createAndSwitch,
			options: {
				conflictBehavior: "allow",
				enabled: canCreateBranch,
				meta: workspaceHotkeys.createBranchAndSwitch.meta,
				requireReset: true,
			},
		},
		{
			hotkey: workspaceHotkeys.fetchFromRemotes.hotkey,
			callback: fetchFromRemotes,
			options: {
				enabled: canFetchFromRemotes,
				meta: workspaceHotkeys.fetchFromRemotes.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.updateWorkspace.hotkey,
			callback: updateWorkspace,
			options: {
				conflictBehavior: "allow",
				enabled: canUpdateWorkspace,
				meta: workspaceHotkeys.updateWorkspace.meta,
			},
		},
		{
			hotkey: "[",
			callback: () => {
				setPage(adjacentPage(page, -1));
			},
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "]",
			callback: () => {
				setPage(adjacentPage(page, 1));
			},
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
	]);

	return (
		<div className={styles.container} ref={ref}>
			<div className={styles.top}>
				<SidebarHeader
					bell={<NotificationBell projectId={projectId} />}
					project={project}
					canFetch={canFetchFromRemotes}
					isFetchPending={isWorkspaceFetchFromRemotesPending}
					lastSuccessfulFetchMs={workspaceFetchStatus?.lastSuccessfulMs}
					onFetch={fetchFromRemotes}
					canOpenSettings={noOperationPending}
					onOpenSettings={openSettings}
					onOpenProjectPicker={openProjectPicker}
				/>

				<ToggleGroup
					render={<ToggleGroupStyles />}
					aria-label="Pages"
					value={[page]}
					onValueChange={selectPage}
				>
					<Toggle
						render={<ToggleStyles />}
						value={"workspace" satisfies PageId}
						aria-label="Workspace"
					>
						<Icon name="workbench" />
						<span className={styles.tabLabel}>Workspace</span>
						<WorkspaceActivityBadge projectId={projectId} />
					</Toggle>
					<Toggle
						render={<ToggleStyles />}
						value={"upstream" satisfies PageId}
						aria-label="Upstream"
					>
						<Icon name="inbox" />
						<span className={styles.tabLabel}>Upstream</span>
						{upstreamList.incomingCount > 0 && (
							<Badge variant="fillGray">
								{upstreamList.incomingCount > maxBadgeCount
									? `${maxBadgeCount}+`
									: upstreamList.incomingCount}
							</Badge>
						)}
					</Toggle>
					<Toggle
						render={<ToggleStyles />}
						value={"branches" satisfies PageId}
						aria-label="Branches"
					>
						<Icon name="branch" />
						<span className={styles.tabLabel}>Branches</span>
					</Toggle>
				</ToggleGroup>
			</div>

			{page === "branches" ? (
				<BranchesList
					className={styles.page}
					projectId={projectId}
					list={branchesList}
					newBranch={newBranch}
				/>
			) : page === "upstream" ? (
				<UpstreamList
					className={styles.page}
					projectId={projectId}
					list={upstreamList}
					canUpdateWorkspace={canUpdateWorkspace}
					isUpdatePending={isWorkspaceIntegrateUpstreamPending}
					onUpdateWorkspace={updateWorkspace}
				/>
			) : (
				<WorkspaceLists
					className={styles.page}
					addressSpace={addressSpace}
					uncommittedAddressSpace={uncommittedAddressSpace}
					absorptionTargetCommitIds={absorptionTargetCommitIds}
					projectId={projectId}
					onActiveFileSelection={onActiveFileSelection}
					stacksHeaderActions={
						<RowToolbar forceVisible>
							<Tooltip.Root>
								<Tooltip.Trigger
									aria-label="New branch"
									className={getRowButtonClassName({ size: "regular", iconOnly: true })}
									onClick={(event) => {
										void showNativeMenuFromTrigger(event.currentTarget, newBranch.menuItems);
									}}
									// We pass `disabled` here because we want to disable the button, not
									// the tooltip. Other props should be passed above.
									render={<Button focusableWhenDisabled disabled={!canCreateBranch} />}
								>
									{newBranch.isPending ? <Icon name="spinner" /> : <Icon name="plus" />}
								</Tooltip.Trigger>
								<Tooltip.Portal>
									<Tooltip.Positioner sideOffset={4}>
										{/* The menu carries both keys; the tooltip names the one that
										    skips it. */}
										<Tooltip.Popup
											render={
												<TooltipPopup kbd={workspaceHotkeys.createIndependentBranch.hotkey} />
											}
										>
											New branch
										</Tooltip.Popup>
									</Tooltip.Positioner>
								</Tooltip.Portal>
							</Tooltip.Root>
						</RowToolbar>
					}
				/>
			)}
		</div>
	);
};
