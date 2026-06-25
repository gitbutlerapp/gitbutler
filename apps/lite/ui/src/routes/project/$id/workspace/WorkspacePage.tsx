import {
	absorptionPlanQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { useRestoreSnapshot } from "#ui/api/mutations.ts";
import {
	focusAdjacentSelectionScope,
	focusSelectionScope,
	getFocusedSelectionScope,
	SelectionScope,
	useOutlineSelection,
} from "#ui/selection-scopes.ts";
import {
	projectActions,
	selectProjectDetailsFullWindow,
	selectProjectDialogState,
	selectProjectFilesVisible,
	selectProjectOutlineModeState,
} from "#ui/projects/state.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { PickerDialog } from "#ui/components/PickerDialog.tsx";
import { globalHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { lastOpenedProjectKey } from "#ui/projects/last-opened.ts";
import { type AppThunk, useAppDispatch, useAppSelector } from "#ui/store.ts";
import { ProjectForFrontend, RefInfo, Segment } from "@gitbutler/but-sdk";
import { useHotkey, useHotkeys } from "@tanstack/react-hotkeys";
import {
	QueryErrorResetBoundary,
	useQueries,
	useQuery,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";
import { Match } from "effect";
import { type FC, Component, ReactNode, useDeferredValue } from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import {
	branchOperand,
	commitOperand,
	operandContains,
	operandEquals,
	operandIdentityKey,
	type BranchOperand,
	type Operand,
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
import { reverse } from "effect/Array";

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "outline-panel" | "details-panel";

const toggleFiles =
	({
		projectId,
		focusedSelectionScope,
		outlineVisible,
	}: {
		projectId: string;
		focusedSelectionScope: SelectionScope | null;
		outlineVisible: boolean;
	}): AppThunk =>
	(dispatch, getState) => {
		const filesVisible = selectProjectFilesVisible(getState(), projectId);

		if (focusedSelectionScope === "files" && filesVisible)
			focusSelectionScope(outlineVisible ? "outline" : "diff");

		dispatch(projectActions.toggleFiles({ projectId }));
	};

const useWorkspaceHotkeys = (projectId: string) => {
	const dispatch = useAppDispatch();
	const detailsFullWindow = useAppSelector((state) =>
		selectProjectDetailsFullWindow(state, projectId),
	);
	const dialog = useAppSelector((state) => selectProjectDialogState(state, projectId));
	const filesVisible = useAppSelector((state) => selectProjectFilesVisible(state, projectId));
	const activeElement = useActiveElement();
	const focusedSelectionScope = getFocusedSelectionScope(activeElement);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
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
				if (dialog._tag === "CommandPalette") dispatch(projectActions.closeDialog({ projectId }));
				else dispatch(projectActions.openCommandPalette({ projectId }));
			},
			options: {
				conflictBehavior: "allow",
				meta: globalHotkeys.commandPalette.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.toggleFiles.hotkey,
			callback: () => {
				dispatch(toggleFiles({ projectId, focusedSelectionScope, outlineVisible }));
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
				meta: workspaceHotkeys.focusPreviousSelectionScope.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.focusNextSelectionScope.hotkey,
			callback: () => {
				focusAdjacentSelectionScope({ filesVisible, offset: 1, outlineVisible });
			},
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.focusNextSelectionScope.meta,
			},
		},
	]);
};

const outlineNavigationItems = (headInfo: RefInfo | undefined): Array<Operand> => {
	const segmentItems = (stackId: string, segment: Segment): Array<Operand> => [
		...(segment.refName
			? [branchOperand({ stackId, branchRef: segment.refName.fullNameBytes })]
			: []),
		...segment.commits.map((commit) => commitOperand({ stackId, commitId: commit.id })),
	];

	return [
		uncommittedChangesOperand,

		...reverse(headInfo?.stacks ?? []).flatMap((stack) => {
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
	absorptionTargetKeys,
}: {
	projectId: string;
	absorptionTargetKeys: ReadonlySet<string>;
}): NavigationIndex<Operand> => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const items = outlineNavigationItems(headInfo);
	const filteredItems = Match.value(outlineMode).pipe(
		Match.tagsExhaustive({
			Default: () => items,
			Absorb: (activeMode) =>
				items.filter(
					(operand) =>
						operandContains(operand, activeMode.source) ||
						absorptionTargetKeys.has(operandIdentityKey(operand)),
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
		window.localStorage.setItem(lastOpenedProjectKey, project.id);
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
		selectProjectDetailsFullWindow(state, projectId),
	);
	const dialog = useAppSelector((state) => selectProjectDialogState(state, projectId));
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	useWorkspaceHotkeys(projectId);

	const selectBranch = (branch: BranchOperand) => {
		dispatch(
			projectActions.selectOutline({
				projectId,
				selection: branchOperand(branch),
			}),
		);
		focusSelectionScope("outline");
	};

	const setBranchPickerOpen = (open: boolean) => {
		if (open) dispatch(projectActions.openBranchPicker({ projectId }));
		else dispatch(projectActions.closeDialog({ projectId }));
	};

	const setApplyBranchPickerOpen = (open: boolean) => {
		if (open) dispatch(projectActions.openApplyBranchPicker({ projectId }));
		else dispatch(projectActions.closeDialog({ projectId }));
	};

	const setCommandPaletteOpen = (open: boolean) => {
		if (open) dispatch(projectActions.openCommandPalette({ projectId }));
		else dispatch(projectActions.closeDialog({ projectId }));
	};

	const setProjectPickerOpen = (open: boolean) => {
		if (open) dispatch(projectActions.openProjectPicker({ projectId }));
		else dispatch(projectActions.closeDialog({ projectId }));
	};

	const openProjectPicker = () => {
		dispatch(projectActions.openProjectPicker({ projectId }));
	};

	const toggleDetailsFullWindow = () => {
		if (
			!detailsFullWindow &&
			getFocusedSelectionScope(document.activeElement) === ("outline" satisfies SelectionScope)
		)
			requestAnimationFrame(() => focusSelectionScope("diff"));

		dispatch(projectActions.toggleDetailsFullWindow({ projectId }));
	};

	useHotkeys([
		{
			hotkey: workspaceHotkeys.toggleDetailsFullWindow.hotkey,
			callback: toggleDetailsFullWindow,
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.toggleDetailsFullWindow.meta,
				ignoreInputs: true,
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
	]);

	const absorptionPlanTarget = Match.value(outlineMode).pipe(
		Match.tag("Absorb", ({ sourceTarget }) => sourceTarget),
		Match.orElse(() => null),
	);
	const [absorptionPlanQuery] = useQueries({
		queries: (absorptionPlanTarget ? [absorptionPlanTarget] : []).map((target) =>
			absorptionPlanQueryOptions({ projectId, target }),
		),
	});
	const absorptionTargetKeys = new Set(
		absorptionPlanQuery?.data?.map(({ stackId, commitId }) =>
			operandIdentityKey(commitOperand({ stackId, commitId })),
		),
	);

	const outlineNavigationIndex = useOutlineNavigationIndex({
		projectId,
		absorptionTargetKeys,
	});

	const outlineSelection = useOutlineSelection({
		projectId,
		navigationIndex: outlineNavigationIndex,
	});

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
			>
				{!detailsFullWindow && (
					<>
						<Panel
							id={"outline-panel" satisfies PanelId}
							className={styles.panel}
							minSize={360}
							defaultSize={400}
							groupResizeBehavior="preserve-pixel-size"
						>
							<Outline
								projectId={projectId}
								project={selectedProject}
								navigationIndex={outlineNavigationIndex}
								absorptionTargetKeys={absorptionTargetKeys}
							/>
						</Panel>
						<Separator className={styles.resizeHandle} />
					</>
				)}

				<Panel id={"details-panel" satisfies PanelId} className={styles.panel}>
					<Details
						key={deferredOutlineSelection ? operandIdentityKey(deferredOutlineSelection) : null}
						style={{ opacity: deferredOutlineSelection !== outlineSelection ? 0.5 : 1 }}
						outlineSelection={deferredOutlineSelection}
						detailsFullWindow={detailsFullWindow}
						onDetailsFullWindowChange={(fullWindow) =>
							dispatch(projectActions.setDetailsFullWindow({ projectId, fullWindow }))
						}
					/>
				</Panel>
			</Group>

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
				}),
			)}
		</>
	);
};

class WorkspacePageErrorBoundary extends Component<
	{
		onReset: () => void;
		children: ReactNode;
	},
	{
		error: Error | null;
	}
> {
	state = { error: null as Error | null };

	static getDerivedStateFromError(error: unknown) {
		return {
			error:
				error instanceof Error
					? error
					: new Error(typeof error === "string" ? error : JSON.stringify(error)),
		};
	}

	handleRetry(): void {
		this.props.onReset();
		this.setState({ error: null });
	}

	render(): ReactNode {
		if (!this.state.error) return this.props.children;

		return (
			<div className={styles.error}>
				<h1 className={styles.errorTitle}>Something went wrong.</h1>
				<div className={styles.errorActions}>
					<button
						type="button"
						className={getButtonClassName({})}
						onClick={() => this.handleRetry()}
					>
						Retry
					</button>
				</div>
				<code className={styles.errorMessage}>{this.state.error.message}</code>
			</div>
		);
	}
}

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
