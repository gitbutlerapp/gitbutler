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
import { getHeadInfoIndex, type HeadInfoIndex } from "#ui/api/ref-info.ts";

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

				dispatch(projectSlice.actions.toggleFiles({ projectId }));
			},
			options: {
				conflictBehavior: "allow",
				enabled: canShowFiles,
				meta: workspaceHotkeys.toggleFiles.meta,
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

const hasAnyOperation = (
	sources: Array<Operand>,
	target: Operand,
	headInfoIndex: HeadInfoIndex,
) => {
	const operations = getOperations(sources, target, headInfoIndex);
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
	absorptionTargetChangeIds,
}: {
	headInfo: RefInfo | undefined;
	outlineMode: OutlineMode;
	absorptionTargetChangeIds: ReadonlySet<string>;
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
						...segment.commits.map((commit) => commitOperand({ changeId: commit.changeId })),
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
						(operand._tag === "Commit" && absorptionTargetChangeIds.has(operand.changeId)),
				),
			Transfer: ({ value: activeMode }) =>
				allItems().filter(
					(operand) =>
						activeMode.sources.some(
							(source) => operandEquals(operand, source) || operandContains(operand, source),
						) ||
						(headInfo && hasAnyOperation(activeMode.sources, operand, getHeadInfoIndex(headInfo))),
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
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : undefined;
	const [absorptionPlanQuery] = useQueries({
		queries: (absorptionPlanTarget ? [absorptionPlanTarget] : []).map((target) =>
			absorptionPlanQueryOptions({ projectId, target }),
		),
	});
	const absorptionTargetChangeIds = new Set(
		headInfoIndex && absorptionPlanQuery?.data
			? absorptionPlanQuery.data.flatMap(({ commitId }) => {
					const changeId = headInfoIndex.commitContextById(commitId)?.commit.changeId;
					return changeId !== undefined ? [changeId] : [];
				})
			: [],
	);

	const outlineNavigationIndex = buildOutlineNavigationIndex({
		headInfo,
		outlineMode,
		absorptionTargetChangeIds,
	});
	const outlineSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionOutline(state, projectId, outlineNavigationIndex),
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
		Match.when("outline", () => outlineSelection),
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
							navigationIndex={outlineNavigationIndex}
							uncommittedFilesNavigationIndex={uncommittedFilesNavigationIndex}
							absorptionTargetChangeIds={absorptionTargetChangeIds}
						/>
					</Panel>
					<Separator className={styles.resizeHandle} />
				</Activity>

				<Panel id={"details-panel" satisfies PanelId} className={styles.panel}>
					<Details
						key={deferredDetailsSelection ? operandIdentityKey(deferredDetailsSelection) : null}
						outlineSelection={deferredDetailsSelection}
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
