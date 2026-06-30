import { useBranchCreate, useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { branchOperand, type BranchOperand, type Operand } from "#ui/operands.ts";
import { projectActions, selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { focusSelectionScope, type SelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import type { NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { Button, Toggle, ToggleGroup, Tooltip } from "@base-ui/react";
import { BottomUpdate, ProjectForFrontend } from "@gitbutler/but-sdk";
import { useIsFetching, useIsMutating, useQuery } from "@tanstack/react-query";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { Match } from "effect";
import { type ComponentProps, type FC } from "react";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { OutlineTree } from "#ui/routes/project/$id/workspace/OutlineTree.tsx";
import styles from "./Outline.module.css";

const isMac = window.lite.platform === "darwin";

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

export const Outline: FC<
	{
		absorptionTargetKeys: ReadonlySet<string>;
		navigationIndex: NavigationIndex<Operand>;
		project: ProjectForFrontend;
		projectId: string;
	} & ComponentProps<"div">
> = ({ absorptionTargetKeys, navigationIndex, project, projectId, ...restProps }) => {
	const dispatch = useAppDispatch();
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const selectBranch = (branch: BranchOperand) => {
		dispatch(
			projectActions.selectOutline({
				projectId,
				selection: branchOperand(branch),
			}),
		);
		focusSelectionScope("outline");
	};

	const openApplyBranchPicker = () => {
		dispatch(projectActions.openApplyBranchPicker({ projectId }));
	};

	const openProjectPicker = () => {
		dispatch(projectActions.openProjectPicker({ projectId }));
	};

	const branchCreateMutation = useBranchCreate();
	const createIndependentBranch = () => {
		branchCreateMutation.mutate(
			{
				projectId,
				newRef: null,
				placement: { type: "independent" },
			},
			{
				onSuccess: (response) => {
					const newBranchStack = getHeadInfoIndex(
						response.workspace.headInfo,
					).branchContextByRefBytes(response.newRef.fullNameBytes)?.stack;

					if (newBranchStack && newBranchStack.id !== null)
						selectBranch({ stackId: newBranchStack.id, branchRef: response.newRef.fullNameBytes });
				},
			},
		);
	};

	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const rebaseUpdates =
		headInfo?.stacks.flatMap((stack): Array<BottomUpdate> => {
			const relativeTo = stackBottomRelativeTo(stack);
			return relativeTo ? [{ kind: "rebase", selector: relativeTo }] : [];
		}) ?? [];
	const workspaceIntegrateUpstreamMutation = useWorkspaceIntegrateUpstream();
	const updateWorkspace = () => {
		workspaceIntegrateUpstreamMutation.mutate({ projectId, updates: rebaseUpdates, dryRun: false });
	};

	// This should be false if all stacks are up-to-date, but we're currently
	// lacking this information:
	// https://linear.app/gitbutler/issue/GB-1560/add-information-about-the-relation-to-the-upstream-to-the-head-info
	const canUpdateWorkspace =
		outlineMode._tag === "Default" &&
		rebaseUpdates.length > 0 &&
		!workspaceIntegrateUpstreamMutation.isPending;

	const canCreateIndependentBranch =
		outlineMode._tag === "Default" && !branchCreateMutation.isPending;

	const canApplyBranch = outlineMode._tag === "Default";

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
				ignoreInputs: true,
				requireReset: true,
			},
		},
		{
			hotkey: workspaceHotkeys.updateWorkspace.hotkey,
			callback: updateWorkspace,
			options: {
				conflictBehavior: "allow",
				enabled: canUpdateWorkspace,
				meta: workspaceHotkeys.updateWorkspace.meta,
				ignoreInputs: true,
			},
		},
	]);

	return (
		<div {...restProps} className={classes(restProps.className, styles.container)}>
			<header className={styles.workspaceControls}>
				<div className={classes(isMac && styles.workspaceControlsMacSpacer)} />
				<div className={styles.workspaceControlsLeft}>
					<button
						type="button"
						className={classes("text-15", "text-bold", styles.workspaceName)}
						onClick={openProjectPicker}
					>
						<span className={styles.workspaceNameLabel}>{project.title}</span>
						<Icon name="chevron-down" className={styles.workspaceNameChevron} />
					</button>
					<ActivitySpinner />
				</div>

				<div className={styles.workspaceControlsActions}>
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
							{branchCreateMutation.isPending ? <Icon name="spinner" /> : <Icon name="plus" />}
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
								<Tooltip.Popup render={<TooltipPopup kbd={workspaceHotkeys.applyBranch.hotkey} />}>
									{workspaceHotkeys.applyBranch.meta.name}
								</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				</div>
			</header>

			<div className={styles.navContainer}>
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
				id={"outline" satisfies SelectionScope}
				data-selection-scope
				tabIndex={0}
				navigationIndex={navigationIndex}
				absorptionTargetKeys={absorptionTargetKeys}
				// Focus on page load.
				ref={(el) => {
					// Don't steal focus if this component is mounted later on.
					if (document.activeElement !== document.body) return;

					el?.focus({ focusVisible: false });
				}}
			/>
		</div>
	);
};
