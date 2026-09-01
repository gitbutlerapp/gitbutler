/**
 * The workspace route's page — the app's hub, and the place to start
 * reading. Everything above (main → App → routes.tsx) is bootstrap,
 * providers and window chrome; everything interesting hangs from here:
 * this file derives every list's address space and data, then renders
 * the pages — each a list — beside Details, and wires the app-level
 * hotkeys and operation controls.
 */
import {
	absorptionPlanQueryOptions,
	changesInWorktreeQueryOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
	operatingModeQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { EditModePage } from "./EditModePage.tsx";
import { useRestoreSnapshot } from "#ui/api/mutations.ts";
import {
	focusHorizontalScope,
	focusScope,
	getFocusedScope,
	useCommittedSelectionFocus,
	type FocusScope,
} from "#ui/focus-scopes.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { PickerDialog } from "#ui/components/PickerDialog.tsx";
import { AddProjectButton } from "#ui/components/AddProjectButton.tsx";
import { useAddLocalRepository } from "#ui/components/useAddLocalRepository.ts";
import { ResizeHandle } from "#ui/components/ResizeHandle.tsx";
import { globalHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { writeLastOpenedProject } from "#ui/project.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import type { ProjectForFrontend } from "@gitbutler/but-sdk";
import { useHotkey, useHotkeys, type UseHotkeyDefinition } from "@tanstack/react-hotkeys";
import {
	QueryErrorResetBoundary,
	useQueries,
	useQuery,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";
import { monitorForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { Match } from "effect";
import {
	type FC,
	Activity,
	useCallback,
	useDeferredValue,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { Group, Panel, useDefaultLayout } from "react-resizable-panels";
import { branchAddress, type BranchAddress, uncommittedChangesFileParent } from "#ui/addresses.ts";
import type { DiffLineSelection } from "#ui/cursors.ts";
import { Details, type DiffViewerHandle, UncommittedFilesDetails } from "./Details.tsx";
import { buildAppliedAddressSpace } from "./applied-address-space.ts";
import { getDiffFileNavigation } from "./diff-view.ts";
import { buildUncommittedFileRows } from "./file-row.ts";
import { fileTreeAddressSpace, selectedFilePath } from "./file-tree.ts";
import { useFileDisplayMode } from "./useFileDisplayMode.ts";
import styles from "./Page.module.css";
import { ApplyBranchPicker } from "./ApplyBranchPicker.tsx";
import { BranchPicker } from "./BranchPicker.tsx";
import { CommandPalette } from "./CommandPalette.tsx";
import { OperationsLogPicker } from "./OperationsLogPicker.tsx";
import { Sidebar } from "./Sidebar.tsx";
import { OperationControls } from "#ui/routes/project/$id/workspace/OperationControls.tsx";
import { ErrorBoundary } from "#ui/components/ErrorBoundary.tsx";
import { Settings } from "./Settings/Settings.tsx";
import { useBranchesList } from "./useBranchesList.ts";
import { upstreamCommitReview, useUpstreamList } from "./useUpstreamList.ts";
import { useStateReconciler as useReconcileState } from "#ui/reconcile.ts";
import { useReviewActivityInbox } from "#ui/review-notifications.ts";
import { useStampReviewsSeen } from "#ui/review-seen.ts";
import {
	setCursor,
	setActiveList,
	cancelPendingOperation,
	useCanShowFiles,
	useSidebarFocusScope,
	usePage,
	useSelection,
	useActiveList,
} from "#ui/use-cursor.ts";
import { defaultSettings } from "#ui/settings.ts";
import { parseDragData } from "./DragData.ts";

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "sidebar-panel" | "details-panel";

const useWorkspaceHotkeys = (projectId: string) => {
	const dispatch = useAppDispatch();
	const store = useAppStore();
	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);
	const dialog = useAppSelector(interfaceSlice.selectors.selectDialogState);
	const canShowFiles = useCanShowFiles();
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const sidebarFocusScope = useSidebarFocusScope();
	const page = usePage();
	const getFilesVisible = () =>
		canShowFiles && projectSlice.selectors.selectFilesVisible(store.getState(), projectId);

	const { isPending: isRestoreSnapshotPending, mutate: restoreSnapshot } = useRestoreSnapshot({
		projectId,
	});

	// Shared by the arrow keys and their h/l aliases so the pairs cannot diverge.
	const focusPane = (offset: -1 | 1) => {
		focusHorizontalScope({
			filesVisible: getFilesVisible(),
			offset,
			sidebarFocusScope,
			detailsFullWindow,
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
			callback: () => restoreSnapshot({ _tag: "redo" }),
			options: {
				enabled: noOperationPending && !isRestoreSnapshotPending,
				meta: globalHotkeys.redo.meta,
				ignoreInputs: true,
			},
		},
		{
			hotkey: globalHotkeys.undo.hotkey,
			callback: () => restoreSnapshot({ _tag: "undo" }),
			options: {
				enabled: noOperationPending && !isRestoreSnapshotPending,
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
			hotkey: globalHotkeys.operationsLog.hotkey,
			callback: () => {
				if (dialog._tag === "OperationsLogPicker") dispatch(interfaceSlice.actions.closeDialog());
				else
					dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "OperationsLogPicker" } }));
			},
			options: {
				enabled: noOperationPending,
				meta: globalHotkeys.operationsLog.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.toggleFiles.hotkey,
			callback: () => {
				if (getFocusedScope(document.activeElement) === "files" && getFilesVisible())
					focusScope(detailsFullWindow ? "diff" : "sidebar");

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
			callback: () => focusScope("details"),
		},
		...Match.value(page).pipe(
			Match.withReturnType<Array<UseHotkeyDefinition>>(),
			Match.when("workspace", () => [
				{
					hotkey: "1",
					callback: () => focusScope("uncommitted-files"),
					options: {
						enabled: !detailsFullWindow,
					},
				},
				{
					hotkey: "2",
					callback: () => focusScope("sidebar"),
					options: {
						enabled: !detailsFullWindow,
					},
				},
			]),
			Match.when("branches", () => [
				{
					hotkey: "1",
					callback: () => focusScope("sidebar"),
					options: {
						enabled: !detailsFullWindow,
					},
				},
			]),
			Match.when("upstream", () => [
				{
					hotkey: "1",
					callback: () => focusScope("sidebar"),
					options: {
						enabled: !detailsFullWindow,
					},
				},
			]),
			Match.exhaustive,
		),
		{
			hotkey: workspaceHotkeys.focusHorizontalScopeLeft.hotkey,
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
			hotkey: workspaceHotkeys.focusHorizontalScopeRight.hotkey,
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

type ProjectPickerProps = {
	open: boolean;
	projects: Array<ProjectForFrontend>;
	selectedProjectId: string;
	onOpenChange: (open: boolean) => void;
	onAddProject: () => void;
	isAddingProject: boolean;
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
			footerAction={
				<AddProjectButton
					size="small"
					isPending={p.isAddingProject}
					onClick={() => {
						p.onOpenChange(false);
						p.onAddProject();
					}}
				/>
			}
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

const PageBody: FC<{ projectId: string }> = ({ projectId }) => {
	useReconcileState(projectId);
	useReviewActivityInbox(projectId);
	useStampReviewsSeen(projectId);

	// A virtualised drag source may unmount before the drag ends, leaving us stuck in a pending
	// operation state. This monitor is essentially a finally block for this scenario; its onDrop runs
	// after those of valid drop targets, in which case it's a no-op.
	useEffect(
		() =>
			monitorForElements({
				canMonitor: ({ source }) => parseDragData(source.data) !== null,
				onDrop: cancelPendingOperation,
			}),
		[],
	);

	const dispatch = useAppDispatch();

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

	// useCallback, not compiler memoisation: the deferred details element below
	// keys on this identity, so it must be stable by construction.
	const onActiveFileSelection = useCallback(
		(itemId: string, firstSelection: DiffLineSelection | null) => {
			setCursor("diff", firstSelection);

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
		},
		[renderAllFiles],
	);

	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);
	const dialog = useAppSelector(interfaceSlice.selectors.selectDialogState);
	const pendingOperation = useAppSelector((state) =>
		projectSlice.selectors.selectPendingOperation(state, projectId),
	);

	useWorkspaceHotkeys(projectId);

	const selectBranch = (branch: BranchAddress) => {
		setCursor("applied", branchAddress(branch));
		focusScope("sidebar");
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

	const setOperationsLogPickerOpen = (open: boolean) => {
		if (open)
			dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "OperationsLogPicker" } }));
		else dispatch(interfaceSlice.actions.closeDialog());
	};

	// Owned here rather than by the picker's footer button: the flow outlives
	// the dialog, which unmounts on close.
	const { addLocalRepository, isPending: isAddingProject } = useAddLocalRepository();

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
			getFocusedScope(document.activeElement) === ("sidebar" satisfies FocusScope)
		)
			requestAnimationFrame(() => focusScope("diff"));

		dispatch(interfaceSlice.actions.toggleDetailsFullWindow());
	};

	useHotkeys([
		{
			hotkey: workspaceHotkeys.toggleSidebar.hotkey,
			callback: toggleDetailsFullWindow,
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.toggleSidebar.meta,
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

	const absorptionPlanTarget = Match.value(pendingOperation).pipe(
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
	const appliedAddressSpace = buildAppliedAddressSpace({
		headInfo,
		pendingOperation,
		absorptionTargetCommitIds,
		foldedSegments,
	});

	const page = usePage();
	const branchesList = useBranchesList(projectId);
	const upstreamList = useUpstreamList(projectId);

	const appliedSelection = useSelection("applied", appliedAddressSpace);
	const branchesSelection = useSelection("unapplied", branchesList.data?.addressSpace);
	const upstreamSelection = useSelection("upstream", upstreamList.addressSpace);

	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const uncommittedFilesFilter = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesFilter(state, projectId),
	);
	const uncommittedFilesDisplayMode = useFileDisplayMode();
	const uncommittedFilesRecentFirst = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesRecentFirst(state, projectId),
	);
	const uncommittedFilesCollapsedDirectories = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesCollapsedDirectories(state, projectId),
	);
	const uncommittedFileRows = buildUncommittedFileRows({
		worktreeChanges,
		filter: uncommittedFilesFilter,
		mode: uncommittedFilesDisplayMode,
		collapsedDirectories: uncommittedFilesCollapsedDirectories,
		recentFirst: uncommittedFilesRecentFirst,
	});
	// Directories take the cursor as files do, so the index follows the layout the
	// list renders — and a collapsed directory takes its files out of it too.
	const uncommittedAddressSpace = fileTreeAddressSpace(uncommittedFileRows);
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
		// Indexed against the worktree changes rather than the address space,
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

		setCursor("uncommitted", selection);
		if (navigation) onActiveFileSelection(navigation.itemId, navigation.firstSelection);
	};

	const uncommittedFilesSelection = useSelection("uncommitted", uncommittedAddressSpace);

	const activeList = useActiveList();
	// The page picks only which list's cursor drives the pane; one Details
	// component then dispatches on the selection itself. The uncommitted arm is
	// the genuine fork — its cursor is a path, not an address. Memoised because
	// `useDeferredValue` compares by identity, so a freshly built element every
	// render would defer every render. Looked up outside the memo so the details
	// only rebuild when the review itself changes, not on every list rerun.
	const upstreamReview =
		upstreamSelection?._tag === "Commit"
			? upstreamCommitReview(upstreamList, upstreamSelection.commitId)
			: null;
	const details = useMemo(() => {
		const viewProps = { projectId, onActiveFileSelection, viewerRef, didScrollToViaFileRef };

		return Match.value(page).pipe(
			Match.when("workspace", () =>
				Match.value(activeList).pipe(
					Match.when("applied", () => (
						<Details selection={appliedSelection} review={null} {...viewProps} />
					)),
					Match.when(
						"uncommitted",
						() =>
							uncommittedFilesSelection !== null && (
								<UncommittedFilesDetails path={uncommittedFilesSelection} {...viewProps} />
							),
					),
					Match.exhaustive,
				),
			),
			Match.when("upstream", () => (
				<Details selection={upstreamSelection} review={upstreamReview} {...viewProps} />
			)),
			Match.when("branches", () => (
				<Details selection={branchesSelection} review={null} {...viewProps} />
			)),
			Match.exhaustive,
		);
	}, [
		projectId,
		branchesSelection,
		onActiveFileSelection,
		appliedSelection,
		page,
		uncommittedFilesSelection,
		upstreamReview,
		upstreamSelection,
		activeList,
	]);

	const deferredDetails = useDeferredValue(details);
	const [focusRestoreRequest, setFocusRestoreRequest] = useState<{ scope: FocusScope } | null>(
		null,
	);
	const consumedFocusRestoreRequest = useRef(focusRestoreRequest);
	// The event callback cannot restore focus itself: the router settles before deferred details do.
	// oxlint-disable react-you-might-not-need-an-effect/no-event-handler
	useEffect(() => {
		if (
			focusRestoreRequest === null ||
			consumedFocusRestoreRequest.current === focusRestoreRequest ||
			deferredDetails !== details
		)
			return;

		focusScope(focusRestoreRequest.scope);
		consumedFocusRestoreRequest.current = focusRestoreRequest;
	}, [deferredDetails, details, focusRestoreRequest]);
	// oxlint-enable react-you-might-not-need-an-effect/no-event-handler

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
		: ["sidebar-panel", "details-panel"];
	const workspaceLayout = useDefaultLayout({
		id: layoutId,
		panelIds,
	});
	const selectionFocus = useCommittedSelectionFocus((scope) => {
		if (page !== "workspace") return;
		if (scope === "uncommitted-files") setActiveList("uncommitted");
		if (scope === "sidebar") setActiveList("applied");
	});

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	return (
		<>
			<Group
				{...selectionFocus}
				id={layoutId}
				className={styles.page}
				defaultLayout={workspaceLayout.defaultLayout}
				onLayoutChanged={workspaceLayout.onLayoutChanged}
				data-selection-focus-styles={
					!(pendingOperation._tag === "Transfer" && pendingOperation.value._tag === "Pointer")
				}
			>
				<Activity mode={detailsFullWindow ? "hidden" : "visible"}>
					<Panel
						id={"sidebar-panel" satisfies PanelId}
						className={styles.panel}
						minSize={260}
						defaultSize={420}
						groupResizeBehavior="preserve-pixel-size"
					>
						{/* No reset key: the child is built inline, so its identity changes
						    every render. Recovery here is the fallback's Retry button. */}
						<ErrorBoundary>
							<Sidebar
								projectId={projectId}
								project={selectedProject}
								branchesList={branchesList}
								upstreamList={upstreamList}
								addressSpace={appliedAddressSpace}
								uncommittedAddressSpace={uncommittedAddressSpace}
								absorptionTargetCommitIds={absorptionTargetCommitIds}
								onActiveFileSelection={onActiveUncommittedFileSelection}
							/>
						</ErrorBoundary>
					</Panel>
					<ResizeHandle />
				</Activity>

				<Panel
					id={"details-panel" satisfies PanelId}
					className={styles.panel}
					data-focus-scope={"details" satisfies FocusScope}
				>
					{/* Keyed on the deferred view itself, not on the URL: the deferred
					    value still holds the old view for a beat after navigating, so a
					    URL key would clear the error onto the element that just threw. */}
					<ErrorBoundary resetKeys={[deferredDetails]}>{deferredDetails}</ErrorBoundary>
				</Panel>
			</Group>

			<OperationControls
				projectId={projectId}
				appliedAddressSpace={appliedAddressSpace}
				onFocusRestore={(scope) => setFocusRestoreRequest({ scope })}
			/>

			{Match.value(dialog).pipe(
				Match.tagsExhaustive({
					None: () => null,
					ApplyBranchPicker: () => (
						<ApplyBranchPicker open onOpenChange={setApplyBranchPickerOpen} projectId={projectId} />
					),
					BranchPicker: () => (
						<BranchPicker
							projectId={projectId}
							open
							onOpenChange={setBranchPickerOpen}
							onSelectBranch={selectBranch}
						/>
					),
					CommandPalette: () => <CommandPalette open onOpenChange={setCommandPaletteOpen} />,
					OperationsLogPicker: () => (
						<OperationsLogPicker
							open
							projectId={projectId}
							onOpenChange={setOperationsLogPickerOpen}
						/>
					),
					ProjectPicker: () => (
						<ProjectPicker
							open
							projects={projects}
							selectedProjectId={projectId}
							onOpenChange={setProjectPickerOpen}
							onAddProject={() => void addLocalRepository()}
							isAddingProject={isAddingProject}
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

export const Page: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const { data: headAndMode } = useQuery(operatingModeQueryOptions(projectId));
	const project = projects.find((project) => project.id === projectId);
	if (!project) return <p className={styles.notFound}>Project not found.</p>;

	// Edit mode is repository state, not navigation: the whole surface swaps
	// while HEAD is parked on the edit ref, and swaps back when it returns —
	// including when the transition happened in a terminal.
	if (headAndMode?.operatingMode.type === "Edit")
		return <EditModePage projectId={projectId} metadata={headAndMode.operatingMode.subject} />;

	return (
		<QueryErrorResetBoundary>
			{({ reset }) => (
				<ErrorBoundary onReset={reset}>
					<PageBody projectId={projectId} />
				</ErrorBoundary>
			)}
		</QueryErrorResetBoundary>
	);
};
