import { useBranchCreate, useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
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
import { branchOperand, type BranchOperand, type Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { formatRelativeTime } from "#ui/time.ts";
import type { NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { Button, Toast, Toggle, ToggleGroup, Tooltip } from "@base-ui/react";
import type { BottomUpdate, ProjectForFrontend } from "@gitbutler/but-sdk";
import { useIsFetching, useIsMutating, useQuery } from "@tanstack/react-query";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { Match } from "effect";
import { type FC, useRef, useState } from "react";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { OutlineTree } from "#ui/routes/project/$id/workspace/OutlineTree/OutlineTree.tsx";
import { BranchesList } from "#ui/routes/project/$id/workspace/BranchesList.tsx";
import type { BranchesOutline } from "#ui/routes/project/$id/workspace/useBranchesOutline.ts";
import { ProjectFolderIcon } from "#ui/routes/project/$id/workspace/ProjectFolderIcon.tsx";
import { UpstreamList } from "#ui/routes/project/$id/workspace/UpstreamList.tsx";
import type { UpstreamOutline } from "#ui/routes/project/$id/workspace/useUpstreamOutline.ts";
import { assert } from "#ui/assert.ts";
import { Badge } from "#ui/components/Badge.tsx";
import type { OutlineTab } from "#ui/projects/project.ts";
import styles from "./Outline.module.css";
import { TopLeftControls } from "#ui/routes/project/$id/workspace/TopLeftControls.tsx";
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
const outlineTabOrder: Array<OutlineTab> = ["workspace", "upstream", "branches"];

const adjacentOutlineTab = (tab: OutlineTab, offset: -1 | 1): OutlineTab => {
	const index = outlineTabOrder.indexOf(tab);
	return assert(
		outlineTabOrder[(index + offset + outlineTabOrder.length) % outlineTabOrder.length],
	);
};

export const Outline: FC<{
	absorptionTargetCommitIds: ReadonlySet<string>;
	branchesOutline: BranchesOutline;
	upstreamOutline: UpstreamOutline;
	navigationIndex: NavigationIndex<Operand>;
	uncommittedFilesNavigationIndex: NavigationIndex<string>;
	onActiveFileSelection: (selection: string) => void;
	project: ProjectForFrontend;
	projectId: string;
}> = ({
	absorptionTargetCommitIds,
	branchesOutline,
	upstreamOutline,
	navigationIndex,
	uncommittedFilesNavigationIndex,
	onActiveFileSelection,
	project,
	projectId,
}) => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);
	const outlineTab = useAppSelector((state) =>
		projectSlice.selectors.selectOutlineTab(state, projectId),
	);

	const selectOutlineTab = (value: Array<OutlineTab>) => {
		const head = value[0];
		if (head === undefined) return;

		dispatch(projectSlice.actions.setOutlineTab({ projectId, tab: head }));
	};

	const selectBranch = (branch: BranchOperand) => {
		dispatch(
			projectSlice.actions.selectOutline({
				projectId,
				selection: branchOperand(branch),
			}),
		);
		focusSelectionScope("outline");
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

	const { isPending: isBranchCreatePending, mutate: branchCreate } = useBranchCreate();
	const createIndependentBranch = () => {
		branchCreate(
			{
				projectId,
				newRef: null,
				placement: { type: "independent" },
			},
			{
				onSuccess: (response) => {
					selectBranch({ branchRef: response.newRef.fullNameBytes });
				},
			},
		);
	};

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

	// This should be false if all stacks are up-to-date, but we're currently
	// lacking this information:
	// https://linear.app/gitbutler/issue/GB-1560/add-information-about-the-relation-to-the-upstream-to-the-head-info
	//
	// A workspace without stacks can still be updated: integrating with no
	// stack updates advances the target base and reparents the workspace
	// commit.
	const emptyWorkspaceBehindTarget =
		headInfo?.stacks.length === 0 && (headInfo.target?.commitsAhead ?? 0) > 0;
	const canUpdateWorkspace =
		isDefaultMode &&
		(rebaseUpdates.length > 0 || emptyWorkspaceBehindTarget) &&
		!isWorkspaceIntegrateUpstreamPending;
	const canFetchFromRemotes = isDefaultMode && !isWorkspaceFetchFromRemotesPending;

	const canCreateIndependentBranch = isDefaultMode && !isBranchCreatePending;

	const canApplyBranch = isDefaultMode;

	const canOpenSettings = isDefaultMode;

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
			callback: createIndependentBranch,
			options: {
				conflictBehavior: "allow",
				enabled: canCreateIndependentBranch,
				meta: workspaceHotkeys.createIndependentBranch.meta,
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
				dispatch(
					projectSlice.actions.setOutlineTab({
						projectId,
						tab: adjacentOutlineTab(outlineTab, -1),
					}),
				);
			},
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "]",
			callback: () => {
				dispatch(
					projectSlice.actions.setOutlineTab({ projectId, tab: adjacentOutlineTab(outlineTab, 1) }),
				);
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
								className={classes(
									getButtonClassName({ variant: "ghost" }),
									"text-15",
									"text-bold",
									styles.workspaceName,
								)}
								onClick={openProjectPicker}
							>
								<ProjectFolderIcon className={styles.workspaceNameFolder} />
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
					aria-label="Navigation"
					value={[outlineTab]}
					onValueChange={selectOutlineTab}
				>
					<Toggle
						render={<ToggleStyles />}
						value={"workspace" satisfies OutlineTab}
						aria-label="Workspace"
					>
						<Icon name="workbench" />
						<span className={styles.tabLabel}>Workspace</span>
					</Toggle>
					<Toggle
						render={<ToggleStyles />}
						value={"upstream" satisfies OutlineTab}
						aria-label="Upstream"
					>
						<Icon name="inbox" />
						<span className={styles.tabLabel}>Upstream</span>
						{upstreamOutline.incomingCount > 0 && (
							<Badge variant="fillGray">{upstreamOutline.incomingCount}</Badge>
						)}
					</Toggle>
					<Toggle
						render={<ToggleStyles />}
						value={"branches" satisfies OutlineTab}
						aria-label="Branches"
					>
						<Icon name="branch" />
						<span className={styles.tabLabel}>Branches</span>
					</Toggle>
				</ToggleGroup>
			</div>

			{outlineTab === "branches" ? (
				<BranchesList
					className={styles.outlineTree}
					projectId={projectId}
					outline={branchesOutline}
				/>
			) : outlineTab === "upstream" ? (
				<UpstreamList
					className={styles.outlineTree}
					projectId={projectId}
					outline={upstreamOutline}
					canUpdateWorkspace={canUpdateWorkspace}
					isUpdatePending={isWorkspaceIntegrateUpstreamPending}
					onUpdateWorkspace={updateWorkspace}
				/>
			) : (
				<OutlineTree
					className={styles.outlineTree}
					navigationIndex={navigationIndex}
					uncommittedFilesNavigationIndex={uncommittedFilesNavigationIndex}
					absorptionTargetCommitIds={absorptionTargetCommitIds}
					projectId={projectId}
					onActiveFileSelection={onActiveFileSelection}
					stacksHeaderActions={
						<RowToolbar forceVisible>
							<Tooltip.Root>
								<Tooltip.Trigger
									aria-label={workspaceHotkeys.createIndependentBranch.meta.name}
									className={getRowButtonClassName({ size: "regular", iconOnly: true })}
									onClick={createIndependentBranch}
									// We pass `disabled` here because we want to disable the button, not
									// the tooltip. Other props should be passed above.
									render={<Button focusableWhenDisabled disabled={!canCreateIndependentBranch} />}
								>
									{isBranchCreatePending ? <Icon name="spinner" /> : <Icon name="plus" />}
								</Tooltip.Trigger>
								<Tooltip.Portal>
									<Tooltip.Positioner sideOffset={4}>
										<Tooltip.Popup
											render={
												<TooltipPopup kbd={workspaceHotkeys.createIndependentBranch.hotkey} />
											}
										>
											{workspaceHotkeys.createIndependentBranch.meta.name}
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
