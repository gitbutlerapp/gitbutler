import {
	absorptionPlanQueryOptions,
	changesInWorktreeQueryOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { useRestoreSnapshot, useSaveGUISettings } from "#ui/api/mutations.ts";
import {
	focusAdjacentSelectionScope,
	focusSelectionScope,
	getFocusedSelectionScope,
	SelectionScope,
} from "#ui/selection-scopes.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { PickerDialog } from "#ui/components/PickerDialog.tsx";
import { globalHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { writeLastOpenedProject } from "#ui/project.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { ProjectForFrontend, RefInfo, WorktreeChanges } from "@gitbutler/but-sdk";
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
	uncommittedChangesOperand,
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
import type { OutlineMode } from "#ui/outline/mode.ts";
import { defaultSettings } from "#ui/settings.ts";

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "outline-panel" | "details-panel";

const useWorkspaceHotkeys = (projectId: string) => {
	const dispatch = useAppDispatch();
	const detailsFullWindow = useAppSelector((state) =>
		projectSlice.selectors.selectDetailsFullWindow(state, projectId),
	);
	const dialog = useAppSelector((state) =>
		projectSlice.selectors.selectDialogState(state, projectId),
	);
	const { data: filesVisibleSetting } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.filesVisible,
	});
	const filesVisible = filesVisibleSetting ?? defaultSettings.filesVisible;
	const { mutate: saveGUISettings } = useSaveGUISettings();
	const activeElement = useActiveElement();
	const focusedSelectionScope = getFocusedSelectionScope(activeElement);
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);
	const outlineVisible = !detailsFullWindow;

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
				if (dialog._tag === "CommandPalette") {
					dispatch(projectSlice.actions.closeDialog({ projectId }));
				} else {
					dispatch(
						projectSlice.actions.openDialog({ projectId, dialog: { _tag: "CommandPalette" } }),
					);
				}
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

				saveGUISettings({ filesVisible: !filesVisible });
			},
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.toggleFiles.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.focusPreviousSelectionScope.hotkey,
			callback: () => {
				focusAdjacentSelectionScope({ filesVisible, offset: -1, outlineVisible });
			},
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: workspaceHotkeys.focusNextSelectionScope.hotkey,
			callback: () => {
				focusAdjacentSelectionScope({ filesVisible, offset: 1, outlineVisible });
			},
			options: {
				conflictBehavior: "allow",
			},
		},
	]);
};

const hasAnyOperation = (source: Operand, target: Operand) => {
	const operations = getOperations(source, target);
	return !!operations.into || !!operations.above || !!operations.below;
};

const buildOutlineNavigationIndex = ({
	headInfo,
	worktreeChanges,
	outlineMode,
	absorptionTargetCommitIds,
}: {
	headInfo: RefInfo | undefined;
	worktreeChanges: WorktreeChanges | undefined;
	outlineMode: OutlineMode;
	absorptionTargetCommitIds: ReadonlySet<string>;
}): NavigationIndex<Operand> => {
	const allItems = () => [
		uncommittedChangesOperand,
		...(worktreeChanges?.changes.map((change) =>
			fileOperand({ parent: uncommittedChangesFileParent, path: change.path }),
		) ?? []),

		...(headInfo?.stacks.toReversed() ?? []).flatMap((stack) =>
			stack.segments.flatMap(
				(segment): Array<Operand> => [
					...(segment.refName ? [branchOperand({ branchRef: segment.refName.fullNameBytes })] : []),
					...segment.commits.map((commit) => commitOperand({ commitId: commit.id })),
				],
			),
		),
	];

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
						operandEquals(operand, activeMode.source) ||
						operandContains(operand, activeMode.source) ||
						hasAnyOperation(activeMode.source, operand),
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
	const dispatch = useAppDispatch();

	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const detailsFullWindow = useAppSelector((state) =>
		projectSlice.selectors.selectDetailsFullWindow(state, projectId),
	);
	const dialog = useAppSelector((state) =>
		projectSlice.selectors.selectDialogState(state, projectId),
	);
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
		if (open)
			dispatch(projectSlice.actions.openDialog({ projectId, dialog: { _tag: "BranchPicker" } }));
		else dispatch(projectSlice.actions.closeDialog({ projectId }));
	};

	const setApplyBranchPickerOpen = (open: boolean) => {
		if (open) {
			dispatch(
				projectSlice.actions.openDialog({ projectId, dialog: { _tag: "ApplyBranchPicker" } }),
			);
		} else {
			dispatch(projectSlice.actions.closeDialog({ projectId }));
		}
	};

	const setCommandPaletteOpen = (open: boolean) => {
		if (open)
			dispatch(projectSlice.actions.openDialog({ projectId, dialog: { _tag: "CommandPalette" } }));
		else dispatch(projectSlice.actions.closeDialog({ projectId }));
	};

	const setProjectPickerOpen = (open: boolean) => {
		if (open)
			dispatch(projectSlice.actions.openDialog({ projectId, dialog: { _tag: "ProjectPicker" } }));
		else dispatch(projectSlice.actions.closeDialog({ projectId }));
	};

	const setSettingsOpen = (open: boolean) => {
		if (open)
			dispatch(projectSlice.actions.openDialog({ projectId, dialog: { _tag: "Settings" } }));
		else dispatch(projectSlice.actions.closeDialog({ projectId }));
	};

	const openProjectPicker = () => {
		dispatch(projectSlice.actions.openDialog({ projectId, dialog: { _tag: "ProjectPicker" } }));
	};

	const toggleDetailsFullWindow = () => {
		if (
			!detailsFullWindow &&
			getFocusedSelectionScope(document.activeElement) === ("outline" satisfies SelectionScope)
		)
			requestAnimationFrame(() => focusSelectionScope("diff"));

		dispatch(projectSlice.actions.toggleDetailsFullWindow({ projectId }));
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
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
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
		worktreeChanges,
		outlineMode,
		absorptionTargetCommitIds,
	});

	const outlineSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionOutline(state, projectId, outlineNavigationIndex),
	);

	const deferredOutlineSelection = useDeferredValue(outlineSelection);

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
						minSize={355}
						defaultSize={400}
						groupResizeBehavior="preserve-pixel-size"
					>
						<Outline
							projectId={projectId}
							project={selectedProject}
							navigationIndex={outlineNavigationIndex}
							absorptionTargetCommitIds={absorptionTargetCommitIds}
						/>
					</Panel>
					<Separator className={styles.resizeHandle} />
				</Activity>

				<Panel id={"details-panel" satisfies PanelId} className={styles.panel}>
					<Details
						key={deferredOutlineSelection ? operandIdentityKey(deferredOutlineSelection) : null}
						outlineSelection={deferredOutlineSelection}
					/>
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
