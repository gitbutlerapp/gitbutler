import {
	absorptionPlanQueryOptions,
	changesInWorktreeQueryOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { useRestoreSnapshot } from "#ui/api/mutations.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import {
	focusHorizontalSelectionScope,
	focusSelectionScope,
	getFocusedSelectionScope,
	type SelectionScope,
} from "#ui/selection-scopes.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { PickerDialog } from "#ui/components/PickerDialog.tsx";
import { ResizeHandle } from "#ui/components/ResizeHandle.tsx";
import { globalHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { writeLastOpenedProject } from "#ui/project.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import type { ProjectForFrontend, RefInfo, TreeChange, WorktreeChanges } from "@gitbutler/but-sdk";
import { useHotkey, useHotkeys, type UseHotkeyDefinition } from "@tanstack/react-hotkeys";
import {
	QueryErrorResetBoundary,
	useQueries,
	useQuery,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";
import { Match } from "effect";
import { type FC, Activity, useDeferredValue, useRef } from "react";
import { Group, Panel, useDefaultLayout } from "react-resizable-panels";
import {
	branchOperand,
	commitOperand,
	fileOperand,
	operandContains,
	operandEquals,
	operandIdentityKey,
	type BranchOperand,
	type HunkOperand,
	type Operand,
	uncommittedChangesFileParent,
} from "#ui/operands.ts";
import { Details, type DiffViewerHandle } from "./Details.tsx";
import { getDiffFileNavigation } from "./diff-view.ts";
import { pathMatchesFilter } from "./file-row.ts";
import {
	buildFileTreeRows,
	fileTreeNavigationIndex,
	selectedFilePath,
	type FileDisplayMode,
	type FileTreeRow,
} from "./file-tree.ts";
import { useFileDisplayMode } from "./useFileDisplayMode.ts";
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
import { Settings } from "./Settings/Settings.tsx";
import { useBranchesOutline } from "./useBranchesOutline.ts";
import { useUpstreamOutline } from "./useUpstreamOutline.ts";
import type { OutlineMode } from "#ui/outline/mode.ts";
import { useStateReconciler as useReconcileState } from "#ui/reconcile.ts";
import { defaultSettings } from "#ui/settings.ts";

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
	const outlineTab = useAppSelector((state) =>
		projectSlice.selectors.selectOutlineTab(state, projectId),
	);

	const { isPending: isRestoreSnapshotPending, mutate: restoreSnapshot } = useRestoreSnapshot({
		projectId,
	});

	// Shared by the arrow keys and their h/l aliases so the pairs cannot diverge.
	const focusPane = (offset: -1 | 1) => {
		focusHorizontalSelectionScope({
			filesVisible,
			offset,
			outlineSelectionScope,
			outlineVisible,
		});
	};
	const focusPaneLeft = () => {
		focusPane(-1);
	};
	const focusPaneRight = () => {
		focusPane(1);
	};

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
			hotkey: "0",
			callback: () => focusSelectionScope("details"),
		},
		...Match.value(outlineTab).pipe(
			Match.withReturnType<Array<UseHotkeyDefinition>>(),
			Match.when("workspace", () => [
				{
					hotkey: "1",
					callback: () => focusSelectionScope("uncommitted-files"),
					options: {
						enabled: outlineVisible,
					},
				},
				{
					hotkey: "2",
					callback: () => focusSelectionScope("outline"),
					options: {
						enabled: outlineVisible,
					},
				},
			]),
			Match.when("branches", () => [
				{
					hotkey: "1",
					callback: () => focusSelectionScope("outline"),
					options: {
						enabled: outlineVisible,
					},
				},
			]),
			Match.when("upstream", () => [
				{
					hotkey: "1",
					callback: () => focusSelectionScope("outline"),
					options: {
						enabled: outlineVisible,
					},
				},
			]),
			Match.exhaustive,
		),
		{
			hotkey: workspaceHotkeys.focusHorizontalSelectionScopeLeft.hotkey,
			callback: focusPaneLeft,
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: "H",
			callback: focusPaneLeft,
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: workspaceHotkeys.focusHorizontalSelectionScopeRight.hotkey,
			callback: focusPaneRight,
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: "L",
			callback: focusPaneRight,
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

const buildUncommittedFileRows = ({
	worktreeChanges,
	filter,
	mode,
	collapsedDirectories,
}: {
	worktreeChanges: WorktreeChanges | undefined;
	filter: string | null;
	mode: FileDisplayMode;
	collapsedDirectories: Record<string, true>;
}): Array<FileTreeRow<TreeChange>> =>
	buildFileTreeRows({
		items:
			worktreeChanges?.changes.filter((change) => pathMatchesFilter(change.path, filter)) ?? [],
		mode,
		collapsedDirectories,
	});

const buildOutlineNavigationIndex = ({
	headInfo,
	outlineMode,
	absorptionTargetCommitIds,
	foldedSegments,
}: {
	headInfo: RefInfo | undefined;
	outlineMode: OutlineMode;
	absorptionTargetCommitIds: ReadonlySet<string>;
	foldedSegments: Record<string, true>;
}): NavigationIndex<Operand> => {
	const allItems = (): Array<Operand> =>
		headInfo?.stacks.toReversed().flatMap((stack) =>
			stack.segments.flatMap((segment): Array<Operand> => {
				// Matches what OutlineTree renders: a folded segment shows a stub
				// in place of its commits, so they are not navigable.
				const folded =
					segment.refName !== null &&
					foldedSegments[decodeBytes(segment.refName.fullNameBytes)] === true;

				return [
					...(segment.refName ? [branchOperand({ branchRef: segment.refName.fullNameBytes })] : []),
					...(folded
						? []
						: segment.commits.map((commit) =>
								commitOperand({ commitId: commit.id, changeId: commit.changeId }),
							)),
				];
			}),
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
	const { data: renderAllFiles } = useSuspenseQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.unidiff ?? defaultSettings.unidiff,
	});

	const viewerRef = useRef<DiffViewerHandle>(null);

	// In the all-in-one view, file selection scrolls to that file, which triggers CodeView's scroll
	// handler and updates file selection again (as per usual scrolling scenario). That latter file
	// selection is based upon the first file visible in the viewport, which may exclude trailing
	// files collectively shorter than the scroll container.
	//
	// The callback doesn't provide any way of knowing what triggered the scroll, so we use this ref
	// to bypass that latter file selection. We could alternatively attempt to pad the scroll
	// container, but that comes with other complexities and tradeoffs.
	const didScrollToViaFileRef = useRef(false);

	const onActiveFileSelection = (itemId: string, firstHunk: HunkOperand | null) => {
		dispatch(
			projectSlice.actions.selectDiff({
				projectId,
				selection: firstHunk,
			}),
		);

		if (renderAllFiles) {
			didScrollToViaFileRef.current = true;
			const viewer = viewerRef.current?.getInstance();
			// Details selection is deferred, so the ref may still point at a viewer without this file.
			if (!viewer?.getItem(itemId)) return;

			viewer.scrollTo({
				type: "item",
				id: itemId,
			});
		}
	};

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

	const foldedSegments = useAppSelector((state) =>
		projectSlice.selectors.selectFoldedSegments(state, projectId),
	);
	const outlineNavigationIndex = buildOutlineNavigationIndex({
		headInfo,
		outlineMode,
		absorptionTargetCommitIds,
		foldedSegments,
	});

	const outlineTab = useAppSelector((state) =>
		projectSlice.selectors.selectOutlineTab(state, projectId),
	);
	const branchesOutline = useBranchesOutline(projectId);
	const upstreamOutline = useUpstreamOutline(projectId);

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
	const upstreamSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionUpstream(
			state,
			projectId,
			upstreamOutline.navigationIndex,
		),
	);

	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const uncommittedFilesFilter = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesFilter(state, projectId),
	);
	const uncommittedFilesDisplayMode = useFileDisplayMode();
	const uncommittedFilesCollapsedDirectories = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesCollapsedDirectories(state, projectId),
	);
	const uncommittedFileRows = buildUncommittedFileRows({
		worktreeChanges,
		filter: uncommittedFilesFilter,
		mode: uncommittedFilesDisplayMode,
		collapsedDirectories: uncommittedFilesCollapsedDirectories,
	});
	// Directories take the cursor as files do, so the index follows the layout the
	// list renders — and a collapsed directory takes its files out of it too.
	const uncommittedFilesNavigationIndex = fileTreeNavigationIndex(uncommittedFileRows);
	const uncommittedTreeChangeDiffs = useQueries({
		queries:
			worktreeChanges?.changes.map((change) =>
				treeChangeDiffsQueryOptions({ projectId, change }),
			) ?? [],
		combine: (results) => {
			if (!worktreeChanges || results.some((result) => result.data === undefined)) return null;

			return results.map((result) => result.data ?? null);
		},
	});

	const onActiveUncommittedFileSelection = (selection: string) => {
		// A directory row stands for the first file below it, so activating a
		// folder still gives the details pane somewhere to go.
		const path = selectedFilePath(uncommittedFileRows, selection);
		// Indexed against the worktree changes rather than the navigation index,
		// which the file filter can narrow out from under them.
		const index = worktreeChanges?.changes.findIndex((change) => change.path === path) ?? -1;
		const change = index === -1 ? undefined : worktreeChanges?.changes[index];
		const treeChangeDiff = index === -1 ? undefined : uncommittedTreeChangeDiffs?.[index];
		const navigation =
			change && treeChangeDiff !== undefined
				? getDiffFileNavigation({
						fileParent: uncommittedChangesFileParent,
						change,
						treeChangeDiff,
					})
				: null;

		dispatch(projectSlice.actions.selectUncommittedFiles({ projectId, selection }));
		if (navigation) onActiveFileSelection(navigation.itemId, navigation.firstHunk);
	};

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
		Match.when("outline", () =>
			Match.value(outlineTab).pipe(
				Match.when("workspace", () => outlineSelection),
				Match.when("upstream", () => upstreamSelection),
				Match.when("branches", () => branchesSelection),
				Match.exhaustive,
			),
		),
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
	const project = projects.find((candidate) => candidate.id === projectId);
	// Names the project group in settings. The route has already established it resolves.
	const projectName = project?.title ?? "";
	// Resolved here rather than in the handler, so the hotkey can disable itself when
	// there is no terminal chosen yet rather than failing on activation.
	const { data: terminalId } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.terminalId ?? "",
	});

	const canOpenTerminal = project !== undefined && terminalId !== undefined && terminalId !== "";
	useHotkey(
		workspaceHotkeys.openInTerminal.hotkey,
		() => {
			if (!canOpenTerminal) return;
			void window.lite.openInTerminal({ terminalId, path: project.path });
		},
		{ enabled: canOpenTerminal, meta: workspaceHotkeys.openInTerminal.meta },
	);

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
						minSize={260}
						defaultSize={420}
						groupResizeBehavior="preserve-pixel-size"
					>
						<Outline
							projectId={projectId}
							project={selectedProject}
							branchesOutline={branchesOutline}
							upstreamOutline={upstreamOutline}
							navigationIndex={outlineNavigationIndex}
							uncommittedFilesNavigationIndex={uncommittedFilesNavigationIndex}
							absorptionTargetCommitIds={absorptionTargetCommitIds}
							onActiveFileSelection={onActiveUncommittedFileSelection}
						/>
					</Panel>
					<ResizeHandle />
				</Activity>

				<Panel
					id={"details-panel" satisfies PanelId}
					className={styles.panel}
					data-selection-scope={"details" satisfies SelectionScope}
				>
					<Details
						selection={deferredDetailsSelection}
						onActiveFileSelection={onActiveFileSelection}
						viewerRef={viewerRef}
						didScrollToViaFileRef={didScrollToViaFileRef}
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
					Settings: () => (
						<Settings
							open
							projectId={projectId}
							projectName={projectName}
							onOpenChange={setSettingsOpen}
						/>
					),
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
