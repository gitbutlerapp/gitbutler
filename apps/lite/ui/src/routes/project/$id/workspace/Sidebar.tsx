import { useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import { setPage, usePage } from "#ui/use-cursor.ts";
import {
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	workspaceFetchQueryOptions,
	workspaceFetchStatusQueryOptions,
} from "#ui/api/queries.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { globalHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import type { Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { formatRelativeTime } from "#ui/time.ts";
import type { NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { Button, Toast, Toggle, ToggleGroup, Tooltip } from "@base-ui/react";
import type { BottomUpdate, ProjectForFrontend } from "@gitbutler/but-sdk";
import { LiteTestId } from "@gitbutler/ui/utils/testIds";
import { useIsFetching, useIsMutating, useQuery } from "@tanstack/react-query";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { Match } from "effect";
import { type FC, useRef, useState } from "react";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { WorkspaceLists } from "#ui/routes/project/$id/workspace/WorkspaceLists/WorkspaceLists.tsx";
import { BranchesList } from "#ui/routes/project/$id/workspace/BranchesList.tsx";
import type { BranchesListData } from "#ui/routes/project/$id/workspace/useBranchesList.ts";
import { FolderIcon } from "#ui/components/FolderIcon.tsx";
import { UpstreamList } from "#ui/routes/project/$id/workspace/UpstreamList.tsx";
import type { UpstreamListData } from "#ui/routes/project/$id/workspace/useUpstreamList.ts";
import { assert } from "#ui/assert.ts";
import { Badge } from "#ui/components/Badge.tsx";
import type { PageId } from "#ui/projects/project.ts";
import styles from "./Sidebar.module.css";
import { TopLeftControls } from "#ui/routes/project/$id/workspace/TopLeftControls.tsx";
import { useNewBranch } from "#ui/routes/project/$id/workspace/useNewBranch.ts";
import { showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { RowToolbar } from "#ui/routes/project/$id/workspace/Row.tsx";
import { getRowButtonClassName } from "#ui/routes/project/$id/workspace/Row-utils.ts";

const ActivitySpinner: FC<{
	/** Suppressed while the fetch button shows its own spinner, to avoid two spinners at once. */
	suppressed: boolean;
}> = (p) => {
	const fetchingCount = useIsFetching();
	const mutatingCount = useIsMutating();

	const isFetching = fetchingCount > 0;
	const isMutating = mutatingCount > 0;

	const status = Match.value({ isFetching, isMutating }).pipe(
		Match.when({ isFetching: true, isMutating: true }, () => "Syncing"),
		Match.when({ isFetching: true }, () => "Loading"),
		Match.when({ isMutating: true }, () => "Saving"),
		Match.orElse(() => null),
	);

	return !p.suppressed && status !== null && <Icon name="spinner" aria-label={status} />;
};

const FetchFromRemotesButton: FC<{
	canFetch: boolean;
	isPending: boolean;
	lastSuccessfulMs?: number | null;
	onFetch: () => void;
}> = (p) => {
	const [tooltipNow, setTooltipNow] = useState(() => Date.now());

	return (
		<Tooltip.Root
			onOpenChange={(open) => {
				if (open) setTooltipNow(Date.now());
			}}
		>
			<Tooltip.Trigger
				aria-label={workspaceHotkeys.fetchFromRemotes.meta.name}
				className={getButtonClassName({ iconOnly: true, variant: "ghost" })}
				onClick={p.onFetch}
				// We pass `disabled` here because we want to disable the button, not
				// the tooltip.
				render={<Button focusableWhenDisabled disabled={!p.canFetch} />}
			>
				<Icon name={p.isPending ? "spinner" : "refresh"} />
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup kbd={workspaceHotkeys.fetchFromRemotes.hotkey} />}>
						{workspaceHotkeys.fetchFromRemotes.meta.name}
						{p.lastSuccessfulMs != null &&
							` (${formatRelativeTime(p.lastSuccessfulMs, tooltipNow)})`}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

/** The tabs in the order they are shown, for cycling with `[` and `]`. */
const pageOrder: Array<PageId> = ["workspace", "upstream", "branches"];

const adjacentPage = (tab: PageId, offset: -1 | 1): PageId => {
	const index = pageOrder.indexOf(tab);
	return assert(pageOrder[(index + offset + pageOrder.length) % pageOrder.length]);
};

export const Sidebar: FC<{
	absorptionTargetCommitIds: ReadonlySet<string>;
	branchesList: BranchesListData;
	upstreamList: UpstreamListData;
	navigationIndex: NavigationIndex<Operand>;
	uncommittedNavigationIndex: NavigationIndex<string>;
	onActiveFileSelection: (selection: string) => void;
	project: ProjectForFrontend;
	projectId: string;
}> = ({
	absorptionTargetCommitIds,
	branchesList,
	upstreamList,
	navigationIndex,
	uncommittedNavigationIndex,
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
	const rebaseUpdates =
		headInfo?.stacks.flatMap((stack): Array<BottomUpdate> => {
			const relativeTo = stackBottomRelativeTo(stack);
			return relativeTo ? [{ kind: "rebase", selector: relativeTo }] : [];
		}) ?? [];
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
		workspaceIntegrateUpstream({ projectId, updates: rebaseUpdates, dryRun: false });
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

	const canApplyBranch = noOperationPending;

	const canOpenSettings = noOperationPending;

	const ref = useRef<HTMLDivElement>(null);

	useHotkeys([
		{
			hotkey: workspaceHotkeys.applyBranch.hotkey,
			callback: openApplyBranchPicker,
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.applyBranch.meta,
				enabled: canApplyBranch,
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
				<header className={styles.workspaceControls}>
					<TopLeftControls />

					<div className={styles.workspaceControlsLeft}>
						<Tooltip.Root>
							<Tooltip.Trigger
								aria-label={`${globalHotkeys.selectProject.meta.name} (current: ${project.title})`}
								data-testid={LiteTestId.ProjectPickerButton}
								className={classes(
									getButtonClassName({ variant: "ghost" }),
									"text-15",
									"text-bold",
									styles.workspaceName,
								)}
								onClick={openProjectPicker}
							>
								<FolderIcon className={styles.workspaceNameFolder} />
								<span className={styles.workspaceNameLabel}>{project.title}</span>
							</Tooltip.Trigger>
							<Tooltip.Portal>
								<Tooltip.Positioner sideOffset={4}>
									<Tooltip.Popup render={<TooltipPopup kbd={globalHotkeys.selectProject.hotkey} />}>
										{globalHotkeys.selectProject.meta.name}
									</Tooltip.Popup>
								</Tooltip.Positioner>
							</Tooltip.Portal>
						</Tooltip.Root>
						<ActivitySpinner suppressed={isWorkspaceFetchFromRemotesPending} />
					</div>

					<div className={styles.workspaceControlsActions}>
						<FetchFromRemotesButton
							canFetch={canFetchFromRemotes}
							isPending={isWorkspaceFetchFromRemotesPending}
							lastSuccessfulMs={workspaceFetchStatus?.lastSuccessfulMs}
							onFetch={fetchFromRemotes}
						/>

						<Tooltip.Root>
							<Tooltip.Trigger
								aria-label={workspaceHotkeys.settings.meta.name}
								className={getButtonClassName({ iconOnly: true, variant: "ghost" })}
								onClick={openSettings}
								// We pass `disabled` here because we want to disable the button, not
								// the tooltip. Other props should be passed above.
								render={<Button focusableWhenDisabled disabled={!canOpenSettings} />}
							>
								<Icon name="settings" />
							</Tooltip.Trigger>
							<Tooltip.Portal>
								<Tooltip.Positioner sideOffset={4}>
									<Tooltip.Popup render={<TooltipPopup kbd={workspaceHotkeys.settings.hotkey} />}>
										{workspaceHotkeys.settings.meta.name}
									</Tooltip.Popup>
								</Tooltip.Positioner>
							</Tooltip.Portal>
						</Tooltip.Root>
					</div>
				</header>

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
					</Toggle>
					<Toggle
						render={<ToggleStyles />}
						value={"upstream" satisfies PageId}
						aria-label="Upstream"
					>
						<Icon name="inbox" />
						<span className={styles.tabLabel}>Upstream</span>
						{upstreamList.incomingCount > 0 && (
							<Badge variant="fillGray">{upstreamList.incomingCount}</Badge>
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
					navigationIndex={navigationIndex}
					uncommittedNavigationIndex={uncommittedNavigationIndex}
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
