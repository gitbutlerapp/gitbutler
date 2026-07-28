import {
	absorptionPlanQueryOptions,
	changesInWorktreeQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { useRestoreSnapshot } from "#ui/api/mutations.ts";
import {
	focusHorizontalSelectionScope,
	focusSelectionScope,
	focusVerticalSelectionScope,
	getFocusedSelectionScope,
	type SelectionScope,
} from "#ui/selection-scopes.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { PickerDialog } from "#ui/components/PickerDialog.tsx";
import { globalHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { writeLastOpenedProject } from "#ui/project.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import type { ProjectForFrontend, RefInfo, WorktreeChanges } from "@gitbutler/but-sdk";
import { useHotkey, useHotkeys } from "@tanstack/react-hotkeys";
import {
	QueryErrorResetBoundary,
	useQueries,
	useQuery,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";
import { Match } from "effect";
import { type FC, Activity, useDeferredValue } from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import {
	branchOperand,
	commitOperand,
	fileOperand,
	operandContains,
	operandEquals,
	operandIdentityKey,
	type BranchOperand,
	type Operand,
	uncommittedChangesFileParent,
} from "#ui/operands.ts";
import { Details } from "./Details.tsx";
import styles from "./WorkspacePage.module.css";
import { useActiveElement } from "#ui/focus.ts";
import { ApplyBranchPicker } from "./ApplyBranchPicker.tsx";
import { BranchPicker } from "./BranchPicker.tsx";
import { CommandPalette } from "./CommandPalette.tsx";
import { Outline } from "./Outline.tsx";
import { getOperations } from "#ui/operations/operation.ts";
import { buildIndexByKey, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { OperationControls } from "#ui/routes/project/$id/workspace/OperationControls.tsx";
import { WorkspacePageErrorBoundary } from "./WorkspacePageErrorBoundary.tsx";
import { Settings } from "./Settings.tsx";
import { useBranchesOutline } from "./useBranchesOutline.ts";
import type { OutlineMode } from "#ui/outline/mode.ts";
import { useStateReconciler as useReconcileState } from "#ui/reconcile.ts";

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "outline-panel" | "details-panel";

const useWorkspaceHotkeys = (projectId: string) => {
	const dispatch = useAppDispatch();
	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);
	const dialog = useAppSelector(interfaceSlice.selectors.selectDialogState);
	const filesVisibleState = useAppSelector((state) =>
		projectSlice.selectors.selectFilesVisible(state, projectId),
	);
	const canShowFiles = useAppSelector((state) =>
		projectSlice.selectors.selectCanShowFiles(state, projectId),
	);
	const activeElement = useActiveElement();
	const focusedSelectionScope = getFocusedSelectionScope(activeElement);
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);
	const outlineVisible = !detailsFullWindow;
	const outlineSelectionScope = useAppSelector((state) =>
		projectSlice.selectors.selectDetailsSelectionScope(state, projectId),
	);
	const filesVisible = canShowFiles && filesVisibleState;

	const { isPending: isRestoreSnapshotPending, mutate: restoreSnapshot } = useRestoreSnapshot({
		projectId,
	});

	useHotkeys([
		{
			hotkey: globalHotkeys.redo.hotkey,
			callback: () => restoreSnapshot("redo"),
			options: {
				enabled: isDefaultMode && !isRestoreSnapshotPending,
				meta: globalHotkeys.redo.meta,
				ignoreInputs: true,
			},
		},
		{
			hotkey: globalHotkeys.undo.hotkey,
			callback: () => restoreSnapshot("undo"),
			options: {
				enabled: isDefaultMode && !isRestoreSnapshotPending,
				meta: globalHotkeys.undo.meta,
				ignoreInputs: true,
			},
		},
		{
			hotkey: globalHotkeys.commandPalette.hotkey,
			callback: () => {
				if (dialog._tag === "CommandPalette") dispatch(interfaceSlice.actions.closeDialog());
				else dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "CommandPalette" } }));
			},
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: workspaceHotkeys.toggleFiles.hotkey,
			callback: () => {
				if (focusedSelectionScope === "files" && filesVisible)
					focusSelectionScope(outlineVisible ? "outline" : "diff");

				dispatch(projectSlice.actions.toggleFiles({ projectId }));
			},
			options: {
				conflictBehavior: "allow",
				enabled: canShowFiles,
				meta: workspaceHotkeys.toggleFiles.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.focusDetails.hotkey,
			callback: () => focusSelectionScope("details"),
		},
		{
			hotkey: workspaceHotkeys.focusUncommittedFiles.hotkey,
			callback: () => focusSelectionScope("uncommitted-files"),
			options: {
				enabled: outlineVisible,
			},
		},
		{
			hotkey: workspaceHotkeys.focusOutline.hotkey,
			callback: () => focusSelectionScope("outline"),
			options: {
				enabled: outlineVisible,
			},
		},
		{
			hotkey: workspaceHotkeys.focusHorizontalSelectionScopeLeft.hotkey,
			callback: () => {
				focusHorizontalSelectionScope({
					filesVisible,
					offset: -1,
					outlineSelectionScope,
					outlineVisible,
				});
			},
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: workspaceHotkeys.focusHorizontalSelectionScopeRight.hotkey,
			callback: () => {
				focusHorizontalSelectionScope({
					filesVisible,
					offset: 1,
					outlineSelectionScope,
					outlineVisible,
				});
			},
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: workspaceHotkeys.focusVerticalSelectionScopeUp.hotkey,
			callback: () => focusVerticalSelectionScope(-1),
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: workspaceHotkeys.focusVerticalSelectionScopeDown.hotkey,
			callback: () => focusVerticalSelectionScope(1),
			options: {
				conflictBehavior: "allow",
			},
		},
	]);
};

const hasAnyOperation = (sources: Array<Operand>, target: Operand) => {
	const operations = getOperations(sources, target);
	return !!operations.into || !!operations.above || !!operations.below;
};

const buildUncommittedFilesNavigationIndex = ({
	worktreeChanges,
}: {
	worktreeChanges: WorktreeChanges | undefined;
}): NavigationIndex<string> => {
	const items = worktreeChanges?.changes.map((change) => change.path) ?? [];
	return { items, indexByKey: buildIndexByKey(items, (path) => path) };
};

const buildOutlineNavigationIndex = ({
	headInfo,
	outlineMode,
	absorptionTargetCommitIds,
}: {
	headInfo: RefInfo | undefined;
	outlineMode: OutlineMode;
	absorptionTargetCommitIds: ReadonlySet<string>;
}): NavigationIndex<Operand> => {
	const allItems = (): Array<Operand> =>
		headInfo?.stacks
			.toReversed()
			.flatMap((stack) =>
				stack.segments.flatMap(
					(segment): Array<Operand> => [
						...(segment.refName
							? [branchOperand({ branchRef: segment.refName.fullNameBytes })]
							: []),
						...segment.commits.map((commit) =>
							commitOperand({ commitId: commit.id, changeId: commit.changeId }),
						),
					],
				),
			) ?? [];

	const filteredItems = Match.value(outlineMode).pipe(
		Match.tagsExhaustive({
			Default: () => allItems(),
			Absorb: (activeMode) =>
				allItems().filter(
					(operand) =>
						operandEquals(operand, activeMode.source) ||
						operandContains(operand, activeMode.source) ||
						(operand._tag === "Commit" && absorptionTargetCommitIds.has(operand.commitId)),
				),
			Transfer: ({ value: activeMode }) =>
				allItems().filter(
					(operand) =>
						activeMode.sources.some(
							(source) => operandEquals(operand, source) || operandContains(operand, source),
						) || hasAnyOperation(activeMode.sources, operand),
				),
			RenameBranch: (x) => [branchOperand(x.operand)],
			RewordCommit: (x) => [commitOperand(x.operand)],
		}),
	);

	const indexByKey = buildIndexByKey(filteredItems, operandIdentityKey);

	return { items: filteredItems, indexByKey };
};

type ProjectPickerProps = {
	open: boolean;
	projects: Array<ProjectForFrontend>;
	selectedProjectId: string;
	onOpenChange: (open: boolean) => void;
};

const ProjectPicker: FC<ProjectPickerProps> = (p) => {
	const navigate = useNavigate();

	const selectProject = (project: ProjectForFrontend) => {
		p.onOpenChange(false);
		void navigate({
			to: "/project/$id/workspace",
			params: { id: project.id },
		});
		writeLastOpenedProject(project.id);
	};

	return (
		<PickerDialog
			ariaLabel="Select project"
			closeLabel="Close project picker"
			emptyLabel="No projects found."
			getItemKey={(project) => project.id}
			getItemLabel={(project) => project.title}
			getItemType={(project) => (project.id === p.selectedProjectId ? "Current" : "Project")}
			itemToStringValue={(project) => project.title}
			items={[
				{
					value: "Projects",
					items: p.projects,
				},
			]}
			open={p.open}
			onOpenChange={p.onOpenChange}
			onSelectItem={selectProject}
			placeholder="Search projects…"
		/>
	);
};

const WorkspacePage: FC = () => {
	useReconcileState();

	const dispatch = useAppDispatch();

	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);
	const dialog = useAppSelector(interfaceSlice.selectors.selectDialogState);
	const outlineMode = useAppSelector((state) =>
		projectSlice.selectors.selectOutlineModeState(state, projectId),
	);

	useWorkspaceHotkeys(projectId);

	const selectBranch = (branch: BranchOperand) => {
		dispatch(
			projectSlice.actions.selectOutline({
				projectId,
				selection: branchOperand(branch),
			}),
		);
		focusSelectionScope("outline");
	};

	const setBranchPickerOpen = (open: boolean) => {
		if (open) dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "BranchPicker" } }));
		else dispatch(interfaceSlice.actions.closeDialog());
	};

	const setApplyBranchPickerOpen = (open: boolean) => {
		if (open)
			dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "ApplyBranchPicker" } }));
		else dispatch(interfaceSlice.actions.closeDialog());
	};

	const setCommandPaletteOpen = (open: boolean) => {
		if (open) dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "CommandPalette" } }));
		else dispatch(interfaceSlice.actions.closeDialog());
	};

	const setProjectPickerOpen = (open: boolean) => {
		if (open) dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "ProjectPicker" } }));
		else dispatch(interfaceSlice.actions.closeDialog());
	};

	const setSettingsOpen = (open: boolean) => {
		if (open) dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "Settings" } }));
		else dispatch(interfaceSlice.actions.closeDialog());
	};

	const openProjectPicker = () => {
		dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "ProjectPicker" } }));
	};

	const toggleDetailsFullWindow = () => {
		if (
			!detailsFullWindow &&
			getFocusedSelectionScope(document.activeElement) === ("outline" satisfies SelectionScope)
		)
			requestAnimationFrame(() => focusSelectionScope("diff"));

		dispatch(interfaceSlice.actions.toggleDetailsFullWindow());
	};

	useHotkeys([
		{
			hotkey: workspaceHotkeys.toggleOutline.hotkey,
			callback: toggleDetailsFullWindow,
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.toggleOutline.meta,
			},
		},
		{
			hotkey: "Escape",
			callback: toggleDetailsFullWindow,
			options: {
				conflictBehavior: "allow",
				enabled: detailsFullWindow,
			},
		},
		{
			hotkey: workspaceHotkeys.settings.hotkey,
			callback: () => setSettingsOpen(dialog._tag !== "Settings"),
		},
	]);

	const absorptionPlanTarget = Match.value(outlineMode).pipe(
		Match.tags({ Absorb: ({ sourceTarget }) => sourceTarget }),
		Match.orElse(() => null),
	);
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const [absorptionPlanQuery] = useQueries({
		queries: (absorptionPlanTarget ? [absorptionPlanTarget] : []).map((target) =>
			absorptionPlanQueryOptions({ projectId, target }),
		),
	});
	const absorptionTargetCommitIds = new Set(
		absorptionPlanQuery?.data?.map(({ commitId }) => commitId),
	);

	const outlineNavigationIndex = buildOutlineNavigationIndex({
		headInfo,
		outlineMode,
		absorptionTargetCommitIds,
	});

	const outlineTab = useAppSelector((state) =>
		projectSlice.selectors.selectOutlineTab(state, projectId),
	);
	const branchesOutline = useBranchesOutline(projectId);

	const outlineSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionOutline(state, projectId, outlineNavigationIndex),
	);
	const branchesSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionBranches(
			state,
			projectId,
			branchesOutline.navigationIndex,
		),
	);

	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const uncommittedFilesNavigationIndex = buildUncommittedFilesNavigationIndex({ worktreeChanges });
	const uncommittedFilesSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionUncommittedFiles(
			state,
			projectId,
			uncommittedFilesNavigationIndex,
		),
	);

	const detailsSelectionScope = useAppSelector((state) =>
		projectSlice.selectors.selectDetailsSelectionScope(state, projectId),
	);
	const detailsSelection = Match.value(detailsSelectionScope).pipe(
		Match.when("outline", () => (outlineTab === "branches" ? branchesSelection : outlineSelection)),
		Match.when("uncommitted-files", () =>
			uncommittedFilesSelection === null
				? null
				: fileOperand({ parent: uncommittedChangesFileParent, path: uncommittedFilesSelection }),
		),
		Match.when(null, () => null),
		Match.exhaustive,
	);

	const deferredDetailsSelection = useDeferredValue(detailsSelection);

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);

	useHotkey(globalHotkeys.selectProject.hotkey, openProjectPicker, {
		enabled: projects.length > 0,
		meta: globalHotkeys.selectProject.meta,
	});

	const layoutId = `project=${projectId}:workspace`;
	const panelIds: Array<PanelId> = detailsFullWindow
		? ["details-panel"]
		: ["outline-panel", "details-panel"];
	const workspaceLayout = useDefaultLayout({
		id: layoutId,
		panelIds,
	});

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	return (
		<>
			<Group
				id={layoutId}
				className={styles.page}
				defaultLayout={workspaceLayout.defaultLayout}
				onLayoutChanged={workspaceLayout.onLayoutChanged}
				data-selection-focus-styles={
					!(outlineMode._tag === "Transfer" && outlineMode.value._tag === "Pointer")
				}
			>
				<Activity mode={detailsFullWindow ? "hidden" : "visible"}>
					<Panel
						id={"outline-panel" satisfies PanelId}
						className={styles.panel}
						minSize={400}
						defaultSize={500}
						groupResizeBehavior="preserve-pixel-size"
					>
						<Outline
							projectId={projectId}
							project={selectedProject}
							branchesOutline={branchesOutline}
							navigationIndex={outlineNavigationIndex}
							uncommittedFilesNavigationIndex={uncommittedFilesNavigationIndex}
							absorptionTargetCommitIds={absorptionTargetCommitIds}
						/>
					</Panel>
					<Separator className={styles.resizeHandle} />
				</Activity>

				<Panel
					id={"details-panel" satisfies PanelId}
					className={styles.panel}
					data-selection-scope={"details" satisfies SelectionScope}
				>
					<Details selection={deferredDetailsSelection} />
				</Panel>
			</Group>

			<OperationControls outlineNavigationIndex={outlineNavigationIndex} />

			{Match.value(dialog).pipe(
				Match.tagsExhaustive({
					None: () => null,
					ApplyBranchPicker: () => (
						<ApplyBranchPicker open onOpenChange={setApplyBranchPickerOpen} projectId={projectId} />
					),
					BranchPicker: () => (
						<BranchPicker open onOpenChange={setBranchPickerOpen} onSelectBranch={selectBranch} />
					),
					CommandPalette: () => <CommandPalette open onOpenChange={setCommandPaletteOpen} />,
					ProjectPicker: () => (
						<ProjectPicker
							open
							projects={projects}
							selectedProjectId={projectId}
							onOpenChange={setProjectPickerOpen}
						/>
					),
					Settings: () => <Settings open onOpenChange={setSettingsOpen} />,
				}),
			)}
		</>
	);
};

export const Route: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const project = projects.find((project) => project.id === projectId);
	if (!project) return <p className={styles.notFound}>Project not found.</p>;

	return (
		<QueryErrorResetBoundary>
			{({ reset }) => (
				<WorkspacePageErrorBoundary onReset={reset}>
					<WorkspacePage />
				</WorkspacePageErrorBoundary>
			)}
		</QueryErrorResetBoundary>
	);
};
