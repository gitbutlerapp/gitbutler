import {
	absorptionPlanQueryOptions,
	changesInWorktreeQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { useRestoreSnapshot } from "#ui/api/mutations.ts";
import { bytesEqual } from "#ui/api/bytes.ts";
import {
	focusAdjacentSelectionScope,
	focusSelectionScope,
	getFocusedSelectionScope,
	SelectionScope,
} from "#ui/selection-scopes.ts";
import {
	CheckedCommitIdsContext,
	CheckedCommitIdsRegistryContext,
} from "#ui/CheckedCommitIdsContext.ts";
import { CommitTargetContext, CommitTargetRegistryContext } from "#ui/CommitTargetContext.ts";
import { DetailsFullWindowContext } from "#ui/DetailsFullWindowContext.ts";
import { DialogContext } from "#ui/DialogContext.ts";
import { FilesVisibleContext, FilesVisibleRegistryContext } from "#ui/FilesVisibleContext.ts";
import {
	HighlightedCommitIdsContext,
	HighlightedCommitIdsRegistryContext,
} from "#ui/HighlightedCommitIdsContext.ts";
import {
	DiffSelectionContext,
	FilesSelectionContext,
	OutlineModeContext,
	OutlineSelectionActionsContext,
	OutlineSelectionContext,
	WorkspaceRegistryContext,
} from "#ui/WorkspaceContext.ts";
import { PickerDialog } from "#ui/components/PickerDialog.tsx";
import { globalHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { writeLastOpenedProject } from "#ui/project.ts";
import { ProjectForFrontend, RefInfo, Segment, type RelativeTo } from "@gitbutler/but-sdk";
import { useHotkey, useHotkeys } from "@tanstack/react-hotkeys";
import {
	QueryErrorResetBoundary,
	useQueries,
	useQuery,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";
import { Match } from "effect";
import { type FC, Activity, use, useDeferredValue } from "react";
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
import { resolveOutlineSelection, workspaceTransitions } from "#ui/workspace.ts";
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

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "outline-panel" | "details-panel";

const useWorkspaceHotkeys = (projectId: string) => {
	const { detailsFullWindow } = use(DetailsFullWindowContext);
	const { dialog, openDialog, closeDialog } = use(DialogContext);
	const { filesVisible, toggleFiles } = use(FilesVisibleContext);
	const activeElement = useActiveElement();
	const focusedSelectionScope = getFocusedSelectionScope(activeElement);
	const { outlineMode } = use(OutlineModeContext);
	const outlineVisible = !detailsFullWindow;

	const restoreSnapshotMutation = useRestoreSnapshot({ projectId });

	useHotkeys([
		{
			hotkey: globalHotkeys.redo.hotkey,
			callback: () => restoreSnapshotMutation.mutate("redo"),
			options: {
				enabled: outlineMode._tag === "Default" && !restoreSnapshotMutation.isPending,
				meta: globalHotkeys.redo.meta,
				ignoreInputs: true,
			},
		},
		{
			hotkey: globalHotkeys.undo.hotkey,
			callback: () => restoreSnapshotMutation.mutate("undo"),
			options: {
				enabled: outlineMode._tag === "Default" && !restoreSnapshotMutation.isPending,
				meta: globalHotkeys.undo.meta,
				ignoreInputs: true,
			},
		},
		{
			hotkey: globalHotkeys.commandPalette.hotkey,
			callback: () => {
				if (dialog._tag === "CommandPalette") closeDialog();
				else openDialog({ _tag: "CommandPalette" });
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

				toggleFiles(projectId);
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

const outlineNavigationItems = ({
	headInfo,
	uncommittedFilePaths,
}: {
	headInfo: RefInfo | undefined;
	uncommittedFilePaths: Array<string>;
}): Array<Operand> => {
	const segmentItems = (stackId: string, segment: Segment): Array<Operand> => [
		...(segment.refName
			? [branchOperand({ stackId, branchRef: segment.refName.fullNameBytes })]
			: []),
		...segment.commits.map((commit) => commitOperand({ stackId, commitId: commit.id })),
	];

	return [
		uncommittedChangesOperand,
		...uncommittedFilePaths.map((path) =>
			fileOperand({ parent: uncommittedChangesFileParent, path }),
		),

		...(headInfo?.stacks.toReversed() ?? []).flatMap((stack) => {
			// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
			const stackId = stack.id!;
			return stack.segments.flatMap((segment) => segmentItems(stackId, segment));
		}),
	];
};

const hasAnyOperation = (source: Operand, target: Operand) => {
	const operations = getOperations(source, target);
	return !!operations.into || !!operations.above || !!operations.below;
};

const useOutlineNavigationIndex = ({
	projectId,
	absorptionTargetCommitIds,
}: {
	projectId: string;
	absorptionTargetCommitIds: ReadonlySet<string>;
}): NavigationIndex<Operand> => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const { outlineMode } = use(OutlineModeContext);

	const items = outlineNavigationItems({
		headInfo,
		uncommittedFilePaths: worktreeChanges?.changes.map((change) => change.path) ?? [],
	});
	const filteredItems = Match.value(outlineMode).pipe(
		Match.tagsExhaustive({
			Default: () => items,
			Absorb: (activeMode) =>
				items.filter(
					(operand) =>
						operandContains(operand, activeMode.source) ||
						(operand._tag === "Commit" && absorptionTargetCommitIds.has(operand.commitId)),
				),
			Transfer: (activeMode) =>
				items.filter(
					(operand) =>
						operandContains(operand, activeMode.value.source) ||
						hasAnyOperation(activeMode.value.source, operand),
				),
			RenameBranch: (x) =>
				items.filter((operand) => operandEquals(operand, branchOperand(x.operand))),
			RewordCommit: (x) =>
				items.filter((operand) => operandEquals(operand, commitOperand(x.operand))),
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

type WorkspacePageContentProps = {
	absorptionTargetCommitIds: ReadonlySet<string>;
	outlineNavigationIndex: NavigationIndex<Operand>;
	projectId: string;
};

// Keep outline selection out of the component that builds the navigation index.
// This lets React Compiler retain the index across selection-only updates.
const WorkspacePageContent: FC<WorkspacePageContentProps> = ({
	absorptionTargetCommitIds,
	outlineNavigationIndex,
	projectId,
}) => {
	const { detailsFullWindow, toggleDetailsFullWindow: toggleDetailsFullWindowState } =
		use(DetailsFullWindowContext);
	const { dialog, openDialog, closeDialog } = use(DialogContext);
	const { outlineMode } = use(OutlineModeContext);
	const { outlineSelection } = use(OutlineSelectionContext);
	const { selectOutline } = use(OutlineSelectionActionsContext);

	useWorkspaceHotkeys(projectId);

	const selectBranch = (branch: BranchOperand) => {
		selectOutline(projectId, branchOperand(branch));
		focusSelectionScope("outline");
	};

	const setBranchPickerOpen = (open: boolean) => {
		if (open) openDialog({ _tag: "BranchPicker" });
		else closeDialog();
	};

	const setApplyBranchPickerOpen = (open: boolean) => {
		if (open) openDialog({ _tag: "ApplyBranchPicker" });
		else closeDialog();
	};

	const setCommandPaletteOpen = (open: boolean) => {
		if (open) openDialog({ _tag: "CommandPalette" });
		else closeDialog();
	};

	const setProjectPickerOpen = (open: boolean) => {
		if (open) openDialog({ _tag: "ProjectPicker" });
		else closeDialog();
	};

	const setSettingsOpen = (open: boolean) => {
		if (open) openDialog({ _tag: "Settings" });
		else closeDialog();
	};

	const openProjectPicker = () => {
		openDialog({ _tag: "ProjectPicker" });
	};

	const toggleDetailsFullWindow = () => {
		if (
			!detailsFullWindow &&
			getFocusedSelectionScope(document.activeElement) === ("outline" satisfies SelectionScope)
		)
			requestAnimationFrame(() => focusSelectionScope("diff"));

		toggleDetailsFullWindowState();
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

	const resolvedOutlineSelection = resolveOutlineSelection(
		outlineSelection,
		outlineNavigationIndex,
	);

	const deferredOutlineSelection = useDeferredValue(resolvedOutlineSelection);

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

const WorkspacePage: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const { outlineMode } = use(OutlineModeContext);
	const absorptionPlanTarget = Match.value(outlineMode).pipe(
		Match.tags({ Absorb: ({ sourceTarget }) => sourceTarget }),
		Match.orElse(() => null),
	);
	const [absorptionPlanQuery] = useQueries({
		queries: (absorptionPlanTarget ? [absorptionPlanTarget] : []).map((target) =>
			absorptionPlanQueryOptions({ projectId, target }),
		),
	});
	const absorptionTargetCommitIds = new Set(
		absorptionPlanQuery?.data?.map(({ commitId }) => commitId),
	);
	const outlineNavigationIndex = useOutlineNavigationIndex({
		projectId,
		absorptionTargetCommitIds,
	});

	return (
		<WorkspacePageContent
			absorptionTargetCommitIds={absorptionTargetCommitIds}
			outlineNavigationIndex={outlineNavigationIndex}
			projectId={projectId}
		/>
	);
};

export const Route: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const [workspace, updateWorkspace] = use(WorkspaceRegistryContext)(projectId);
	const outlineSelectionContext: OutlineSelectionContext = {
		outlineSelection: workspace.selection.outline,
	};
	const outlineSelectionActionsContext: OutlineSelectionActionsContext = {
		selectOutline: (projectId, selection) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.selectOutline(workspace, selection),
			),
		updateRewrittenCommitReferences: (projectId, replacedCommits, headInfo) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.updateRewrittenCommitReferences(workspace, replacedCommits, headInfo),
			),
		updateRewrittenBranchReferences: (projectId, oldBranch, newBranch) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.updateRewrittenBranchReferences(workspace, oldBranch, newBranch),
			),
	};
	const filesSelectionContext: FilesSelectionContext = {
		filesSelection: workspace.selection.files,
		selectFiles: (projectId, selection) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.selectFiles(workspace, selection),
			),
	};
	const diffSelectionContext: DiffSelectionContext = {
		diffSelection: workspace.selection.diff,
		selectDiff: (projectId, selection) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.selectDiff(workspace, selection),
			),
	};
	const outlineModeContext: OutlineModeContext = {
		outlineMode: workspace.mode,
		startRewordCommit: (projectId, commit) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.startRewordCommit(workspace, commit),
			),
		startRenameBranch: (projectId, branch) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.startRenameBranch(workspace, branch),
			),
		enterTransferMode: (projectId, mode) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.enterTransferMode(workspace, mode),
			),
		enterKeyboardTransferMode: (projectId, source, operationType) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.enterKeyboardTransferMode(workspace, source, operationType),
			),
		enterAbsorbMode: (projectId, source, sourceTarget) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.enterAbsorbMode(workspace, source, sourceTarget),
			),
		updatePointerTransfer: (projectId, target, operationType) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.updatePointerTransfer(workspace, target, operationType),
			),
		updateTransferOperationType: (projectId, operationType) =>
			updateWorkspace(projectId, (workspace) =>
				workspaceTransitions.updateTransferOperationType(workspace, operationType),
			),
		exitMode: (projectId) => updateWorkspace(projectId, workspaceTransitions.exitMode),
		cancelMode: (projectId) => updateWorkspace(projectId, workspaceTransitions.cancelMode),
	};

	const filesVisibleContext = use(FilesVisibleRegistryContext)(projectId);
	const [commitTarget, updateCommitTarget] = use(CommitTargetRegistryContext)(projectId);
	const commitTargetContext = {
		commitTarget,
		setCommitTarget: (projectId: string, commitTarget: RelativeTo | null) =>
			updateCommitTarget(projectId, () => commitTarget),
		updateRewrittenCommitReferences: (projectId: string, replacedCommits: Record<string, string>) =>
			updateCommitTarget(projectId, (current) => {
				if (current?.type !== "commit") return current;
				const commitId = replacedCommits[current.subject];
				return commitId === undefined || commitId === current.subject
					? current
					: { type: "commit", subject: commitId };
			}),
		updateRewrittenBranchReferences: (
			projectId: string,
			oldBranch: BranchOperand,
			newBranch: BranchOperand,
		) =>
			updateCommitTarget(projectId, (current) =>
				current?.type === "referenceBytes" && bytesEqual(current.subject, oldBranch.branchRef)
					? { type: "referenceBytes", subject: newBranch.branchRef }
					: current,
			),
	};

	const [highlightedCommitIds, setHighlightedCommitIds] = use(HighlightedCommitIdsRegistryContext)(
		projectId,
	);
	const highlightedCommitIdsContext = {
		highlightedCommitIds,
		setHighlightedCommitIds: (projectId: string, commitIds: Array<string>) =>
			setHighlightedCommitIds(projectId, (current) => {
				const next = new Set(commitIds);
				return next.size === current.size && next.isSubsetOf(current) ? current : next;
			}),
		clearHighlightedCommitIds: (projectId: string) =>
			setHighlightedCommitIds(projectId, () => new Set()),
	};

	const [checkedCommitIds, setCheckedCommitIds] = use(CheckedCommitIdsRegistryContext)(projectId);
	const checkedCommitIdsContext = {
		checkedCommitIds,
		setCommitsChecked: (projectId: string, commitIds: Array<string>, checked: boolean) =>
			setCheckedCommitIds(projectId, (current) => {
				const toggled = new Set(commitIds);
				return checked ? current.union(toggled) : current.difference(toggled);
			}),
		clearCheckedCommits: (projectId: string) => setCheckedCommitIds(projectId, () => new Set()),
		updateRewrittenCommitReferences: (projectId: string, replacedCommits: Record<string, string>) =>
			setCheckedCommitIds(projectId, (current) => {
				let next: Set<string> | undefined;
				for (const id of current) {
					const newId = replacedCommits[id];
					if (newId === undefined || newId === id) continue;

					if (next === undefined) next = new Set(current);
					next.delete(id);
					next.add(newId);
				}
				return next ?? current;
			}),
	};

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const project = projects.find((project) => project.id === projectId);
	if (!project) return <p className={styles.notFound}>Project not found.</p>;

	return (
		<OutlineModeContext value={outlineModeContext}>
			<OutlineSelectionActionsContext value={outlineSelectionActionsContext}>
				<OutlineSelectionContext value={outlineSelectionContext}>
					<FilesSelectionContext value={filesSelectionContext}>
						<DiffSelectionContext value={diffSelectionContext}>
							<CommitTargetContext value={commitTargetContext}>
								<HighlightedCommitIdsContext value={highlightedCommitIdsContext}>
									<CheckedCommitIdsContext value={checkedCommitIdsContext}>
										<FilesVisibleContext value={filesVisibleContext}>
											<QueryErrorResetBoundary>
												{({ reset }) => (
													<WorkspacePageErrorBoundary onReset={reset}>
														<WorkspacePage />
													</WorkspacePageErrorBoundary>
												)}
											</QueryErrorResetBoundary>
										</FilesVisibleContext>
									</CheckedCommitIdsContext>
								</HighlightedCommitIdsContext>
							</CommitTargetContext>
						</DiffSelectionContext>
					</FilesSelectionContext>
				</OutlineSelectionContext>
			</OutlineSelectionActionsContext>
		</OutlineModeContext>
	);
};
