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
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { formatRelativeTime } from "#ui/time.ts";
import type { NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { Button, Toast, Toggle, ToggleGroup, Tooltip } from "@base-ui/react";
import { BottomUpdate, ProjectForFrontend } from "@gitbutler/but-sdk";
import { useIsFetching, useIsMutating, useQuery } from "@tanstack/react-query";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { Match } from "effect";
import { type ComponentProps, type FC, useState } from "react";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { OutlineTree } from "#ui/routes/project/$id/workspace/OutlineTree/OutlineTree.tsx";
import styles from "./Outline.module.css";
import { TopLeftControls } from "#ui/routes/project/$id/workspace/TopLeftControls.tsx";

const ActivitySpinner: FC = () => {
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

	return status !== null && <Icon name="spinner" aria-label={status} />;
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
				className={getButtonClassName({ iconOnly: true })}
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

export const Outline: FC<
	{
		absorptionTargetChangeIds: Set<string>;
		navigationIndex: NavigationIndex<Operand>;
		uncommittedFilesNavigationIndex: NavigationIndex<string>;
		project: ProjectForFrontend;
		projectId: string;
	} & ComponentProps<"div">
> = ({
	absorptionTargetChangeIds,
	navigationIndex,
	uncommittedFilesNavigationIndex,
	project,
	projectId,
	...restProps
}) => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);

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
		dispatch(projectSlice.actions.openDialog({ projectId, dialog: { _tag: "ApplyBranchPicker" } }));
	};

	const openProjectPicker = () => {
		dispatch(projectSlice.actions.openDialog({ projectId, dialog: { _tag: "ProjectPicker" } }));
	};

	const openSettings = () => {
		dispatch(projectSlice.actions.openDialog({ projectId, dialog: { _tag: "Settings" } }));
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
	const { data: guiSettings } = useQuery(guiSettingsQueryOptions);
	const { data: workspaceFetchStatus } = useQuery(workspaceFetchStatusQueryOptions(projectId));
	const rebaseUpdates =
		headInfo?.stacks.flatMap((stack): Array<BottomUpdate> => {
			const relativeTo = stackBottomRelativeTo(stack);
			return relativeTo ? [{ kind: "rebase", selector: relativeTo }] : [];
		}) ?? [];
	const { isPending: isWorkspaceIntegrateUpstreamPending, mutate: workspaceIntegrateUpstream } =
		useWorkspaceIntegrateUpstream();
	const { isFetching: isWorkspaceFetchFromRemotesPending, refetch: workspaceFetchFromRemotes } =
		useQuery(workspaceFetchQueryOptions(projectId, guiSettings?.autoFetchFrequency));
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
	const canUpdateWorkspace =
		isDefaultMode && rebaseUpdates.length > 0 && !isWorkspaceIntegrateUpstreamPending;
	const canFetchFromRemotes = isDefaultMode && !isWorkspaceFetchFromRemotesPending;

	const canCreateIndependentBranch = isDefaultMode && !isBranchCreatePending;

	const canApplyBranch = isDefaultMode;

	const canOpenSettings = isDefaultMode;

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
	]);

	return (
		<div {...restProps} className={classes(restProps.className, styles.container)}>
			<div className={styles.top}>
				<header className={styles.workspaceControls}>
					<TopLeftControls />

					<div className={styles.workspaceControlsLeft}>
						<Tooltip.Root>
							<Tooltip.Trigger
								aria-label={globalHotkeys.selectProject.meta.name}
								className={classes("text-15", "text-bold", styles.workspaceName)}
								onClick={openProjectPicker}
							>
								<span className={styles.workspaceNameLabel}>{project.title}</span>
								<Icon name="chevron-down" className={styles.workspaceNameChevron} />
							</Tooltip.Trigger>
							<Tooltip.Portal>
								<Tooltip.Positioner sideOffset={4}>
									<Tooltip.Popup render={<TooltipPopup kbd={globalHotkeys.selectProject.hotkey} />}>
										{globalHotkeys.selectProject.meta.name}
									</Tooltip.Popup>
								</Tooltip.Positioner>
							</Tooltip.Portal>
						</Tooltip.Root>
						<ActivitySpinner />
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
								aria-label={workspaceHotkeys.updateWorkspace.meta.name}
								className={getButtonClassName({ iconOnly: true })}
								onClick={updateWorkspace}
								// We pass `disabled` here because we want to disable the button, not
								// the tooltip. Other props should be passed above.
								render={<Button focusableWhenDisabled disabled={!canUpdateWorkspace} />}
							>
								<Icon name="arrow-line-down" />
							</Tooltip.Trigger>
							<Tooltip.Portal>
								<Tooltip.Positioner sideOffset={4}>
									<Tooltip.Popup
										render={<TooltipPopup kbd={workspaceHotkeys.updateWorkspace.hotkey} />}
									>
										{workspaceHotkeys.updateWorkspace.meta.name}
									</Tooltip.Popup>
								</Tooltip.Positioner>
							</Tooltip.Portal>
						</Tooltip.Root>

						<Tooltip.Root>
							<Tooltip.Trigger
								aria-label={workspaceHotkeys.createIndependentBranch.meta.name}
								className={getButtonClassName({ iconOnly: true })}
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
										render={<TooltipPopup kbd={workspaceHotkeys.createIndependentBranch.hotkey} />}
									>
										{workspaceHotkeys.createIndependentBranch.meta.name}
									</Tooltip.Popup>
								</Tooltip.Positioner>
							</Tooltip.Portal>
						</Tooltip.Root>

						<Tooltip.Root>
							<Tooltip.Trigger
								aria-label={workspaceHotkeys.applyBranch.meta.name}
								className={getButtonClassName({ iconOnly: true })}
								onClick={openApplyBranchPicker}
								// We pass `disabled` here because we want to disable the button, not
								// the tooltip. Other props should be passed above.
								render={<Button focusableWhenDisabled disabled={!canApplyBranch} />}
							>
								<Icon name="branch" />
							</Tooltip.Trigger>
							<Tooltip.Portal>
								<Tooltip.Positioner sideOffset={4}>
									<Tooltip.Popup
										render={<TooltipPopup kbd={workspaceHotkeys.applyBranch.hotkey} />}
									>
										{workspaceHotkeys.applyBranch.meta.name}
									</Tooltip.Popup>
								</Tooltip.Positioner>
							</Tooltip.Portal>
						</Tooltip.Root>

						<Tooltip.Root>
							<Tooltip.Trigger
								aria-label={workspaceHotkeys.settings.meta.name}
								className={getButtonClassName({ iconOnly: true })}
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

				<ToggleGroup render={<ToggleGroupStyles />} aria-label="Navigation" value={["workspace"]}>
					<Toggle render={<ToggleStyles />} value="workspace">
						<Icon name="workbench" />
						Workspace
					</Toggle>
					<Toggle render={<ToggleStyles />} value="upstream" disabled>
						<Icon name="inbox" />
						Upstream
					</Toggle>
					<Toggle render={<ToggleStyles />} value="branches" disabled>
						<Icon name="branch" />
						Branches
					</Toggle>
				</ToggleGroup>
			</div>

			<OutlineTree
				className={styles.outlineTree}
				navigationIndex={navigationIndex}
				uncommittedFilesNavigationIndex={uncommittedFilesNavigationIndex}
				absorptionTargetChangeIds={absorptionTargetChangeIds}
				projectId={projectId}
			/>
		</div>
	);
};
