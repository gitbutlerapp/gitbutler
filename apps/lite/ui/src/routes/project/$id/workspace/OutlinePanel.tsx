import uiStyles from "#ui/components/ui.module.css";
import {
	absorptionPlanQueryOptions,
	changesInWorktreeQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { findCommit, renameBranchInHeadInfo, resolveRelativeTo } from "#ui/api/ref-info.ts";
import { decodeRefName, encodeRefName } from "#ui/api/ref-name.ts";
import { commitIsDiverged, commitTitle, shortCommitId } from "#ui/commit.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import {
	branchOperand,
	changesSectionOperand,
	commitOperand,
	operandEquals,
	operandIdentityKey,
	stackOperand,
	type BranchOperand,
	type CommitOperand,
	type Operand,
} from "#ui/operands.ts";
import { Button } from "#ui/components/Button.tsx";
import {
	filterNavigationIndexForOutlineMode,
	getTransferOperation,
	keyboardTransferOperationMode,
	getOperationSource,
} from "#ui/outline/mode.ts";
import { focusPanel, useFocusedProjectPanel, useNavigationIndexHotkeys } from "#ui/panels.ts";
import {
	projectActions,
	selectProjectCommitTarget,
	selectProjectHighlightedCommitIds,
	selectProjectOutlineModeState,
	selectProjectSelectionOutline,
} from "#ui/projects/state.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { OperationSourceLabel } from "#ui/routes/project/$id/workspace/OperationSourceLabel.tsx";
import { OperationTarget } from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import {
	buildNavigationIndex,
	navigationIndexIncludes,
	Section,
	type NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import { mergeProps, Toast, useRender } from "@base-ui/react";
import { Combobox } from "@base-ui/react/combobox";
import { Toolbar } from "@base-ui/react/toolbar";
import {
	AbsorptionTarget,
	BranchReference,
	Commit,
	InsertSide,
	RefInfo,
	RelativeTo,
	Segment,
	Snapshot,
	Stack,
	TreeChange,
	WorkspaceState,
} from "@gitbutler/but-sdk";
import {
	formatForDisplay,
	useHotkey,
	UseHotkeyDefinition,
	useHotkeys,
	useKeyHold,
} from "@tanstack/react-hotkeys";
import {
	useIsFetching,
	useIsMutating,
	useMutation,
	useQueries,
	useQuery,
	useQueryClient,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match } from "effect";

import {
	ComponentProps,
	createContext,
	FC,
	Fragment,
	SubmitEventHandler,
	use,
	useEffect,
	useOptimistic,
	useRef,
	useState,
	useTransition,
} from "react";
import { Panel, PanelProps } from "react-resizable-panels";
import styles from "./OutlinePanel.module.css";
import workspaceItemRowStyles from "./WorkspaceItemRow.module.css";
import { WorkspaceItemRow, WorkspaceItemRowToolbar } from "./WorkspaceItemRow.tsx";
import { useDryRunOperation } from "#ui/operations/operation.ts";
import { isNonEmptyArray, NonEmptyArray } from "effect/Array";
import { defaultOutlineSelection } from "#ui/projects/workspace/state.ts";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import { Tooltip } from "#ui/components/Tooltip.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { rejectedChangesToastOptions } from "#ui/operations/rejectedChangesToastOptions.tsx";
import {
	changesHotkeys,
	outlineHotkeys,
	toElectronAccelerator,
	workspaceHotkeys,
} from "#ui/hotkeys.ts";
import { DropdownButton } from "#ui/components/DropdownButton.tsx";
import { assert } from "#ui/assert.ts";
import { errorMessageForToast } from "#ui/errors.ts";

const NavigationIndexContext = createContext<NavigationIndex | null>(null);

const DryRunWorkspaceContext = createContext<WorkspaceState | null>(null);

const AbsorptionTargetKeysContext = createContext<ReadonlySet<string> | null>(null);

const useDryRunCommit = (commitId: string) => {
	const dryRunWorkspace = use(DryRunWorkspaceContext);
	if (!dryRunWorkspace) return null;

	const dryRunCommitId = dryRunWorkspace.replacedCommits[commitId] ?? commitId;
	return findCommit({ headInfo: dryRunWorkspace.headInfo, commitId: dryRunCommitId });
};

const sections = (headInfo: RefInfo | undefined): NonEmptyArray<Section> => {
	const changesSection: Section = {
		section: changesSectionOperand,
		children: [],
	};

	const segmentChildren = (stackId: string, segment: Segment): Array<Operand> =>
		segment.commits.map((commit) => commitOperand({ stackId, commitId: commit.id }));

	const segmentSection = (stackId: string, segment: Segment): Section | null => {
		const children = segmentChildren(stackId, segment);
		const branchRef = segment.refName?.fullNameBytes;
		if (!branchRef && children.length === 0) return null;

		return {
			section: branchRef ? branchOperand({ stackId, branchRef }) : null,
			children,
		};
	};

	return [
		changesSection,

		...(headInfo?.stacks.flatMap((stack) => {
			// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
			const stackId = stack.id!;
			const stackOperandSection: Section = {
				section: stackOperand({ stackId }),
				children: [],
			};
			return [
				stackOperandSection,
				...stack.segments.flatMap((segment) => {
					const section = segmentSection(stackId, segment);
					return section ? [section] : [];
				}),
			];
		}) ?? []),
	];
};

const useNavigationIndex = ({
	projectId,
	absorptionTargetKeys,
}: {
	projectId: string;
	absorptionTargetKeys: ReadonlySet<string>;
}) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));

	const dispatch = useAppDispatch();

	const navigationIndexUnfiltered = buildNavigationIndex(sections(headInfo));

	const selection = useAppSelector((state) => selectProjectSelectionOutline(state, projectId));

	// React allows state updates on render, but not for external stores.
	// https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
	useEffect(() => {
		//
		// Reset selection when it's no longer part of the workspace.
		//
		if (!navigationIndexIncludes(navigationIndexUnfiltered, selection))
			dispatch(
				projectActions.selectOutline({
					projectId,
					selection: defaultOutlineSelection,
				}),
			);
	}, [navigationIndexUnfiltered, selection, projectId, dispatch]);

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	return filterNavigationIndexForOutlineMode({
		navigationIndex: navigationIndexUnfiltered,
		outlineMode,
		absorptionTargetKeys,
	});
};

const useOutlineTreeHotkeys = ({
	navigationIndex,
	projectId,
}: {
	navigationIndex: NavigationIndex;
	projectId: string;
}) => {
	const selection = useAppSelector((state) => selectProjectSelectionOutline(state, projectId));
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const dispatch = useAppDispatch();

	const select = (newItem: Operand) =>
		dispatch(projectActions.selectOutline({ projectId, selection: newItem }));

	const toastManager = Toast.useToastManager();
	const commitMoveMutation = useMutation({
		mutationFn: window.lite.commitMove,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to move commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});

	const openBranchPicker = () => {
		dispatch(projectActions.openBranchPicker({ projectId }));
	};

	const enterAbsorbMode = (source: Operand, sourceTarget: AbsorptionTarget) => {
		dispatch(projectActions.enterAbsorbMode({ projectId, source, sourceTarget }));
	};

	const amendCommit = () => {
		dispatch(
			projectActions.enterTransferMode({
				projectId,
				mode: keyboardTransferOperationMode({
					source: changesSectionOperand,
					operationType: "rub",
				}),
			}),
		);
		focusPanel("outline");
	};

	const composeCommitHere = (relativeTo: RelativeTo) => {
		dispatch(projectActions.setCommitTarget({ projectId, commitTarget: relativeTo }));
		focusCommitMessageInput();
	};

	const moveSelectedCommit = (offset: -1 | 1) => {
		if (selection._tag !== "Commit") return;

		const source = commitOperand(selection);
		const selectionIdx = navigationIndex.indexByKey.get(operandIdentityKey(source));
		if (selectionIdx === undefined) return;

		const nextItem = navigationIndex.items[selectionIdx + offset];
		if (!nextItem) return;

		const relativeTo = Match.value(nextItem).pipe(
			Match.tags({
				Commit: ({ commitId }): RelativeTo => ({ type: "commit", subject: commitId }),
				Branch: ({ branchRef }): RelativeTo => ({
					type: "referenceBytes",
					subject: branchRef,
				}),
			}),
			Match.orElse(() => null),
		);
		if (!relativeTo) return;

		commitMoveMutation.mutate({
			projectId,
			subjectCommitIds: [selection.commitId],
			relativeTo,
			side: offset === -1 ? "above" : "below",
			dryRun: false,
		});
	};

	const restoreSnapshotMutation = useMutation({
		mutationFn: async (direction: "redo" | "undo"): Promise<Snapshot | null> => {
			const snapshot =
				direction === "redo"
					? await window.lite.getRedoTargetSnapshot(projectId)
					: await window.lite.getUndoTargetSnapshot(projectId);
			if (!snapshot) return null;

			const [peeled] = await Promise.all([
				window.lite.peelRestoreSnapshot({ projectId, sha: snapshot.commitId }),

				window.lite.restoreSnapshotWithKind({
					projectId,
					restoreKind:
						direction === "redo" ? "RestoreFromSnapshotViaRedo" : "RestoreFromSnapshotViaUndo",
					sha: snapshot.commitId,
				}),
			]);

			return peeled ?? snapshot;
		},
		onSuccess: (snapshot, direction) => {
			const title = direction === "redo" ? "Redo" : "Undo";

			if (!snapshot) {
				toastManager.add({ type: "warning", title, description: `Nothing to ${direction}` });
				return;
			}

			// TODO: We should map this to something user-friendly.
			const op = snapshot.details?.operation;

			// TODO: We should use dynamic units.
			const minsAgo = new Intl.RelativeTimeFormat(undefined, { style: "short" }).format(
				Math.ceil((snapshot.createdAt - Date.now()) / 1000 / 60),
				"minutes",
			);

			toastManager.add({
				type: "info",
				title,
				description: `Restored to ${shortCommitId(snapshot.commitId)} (${op !== undefined ? `${op}, ` : ""}${minsAgo})`,
			});
		},
		onError: (error, direction) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: `Failed to ${direction}`,
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});

	const defaultOutlineHotkeysEnabled = focusedPanel === "outline" && outlineMode._tag === "Default";
	const isSelectedCommit = selection._tag === "Commit";
	const isSelectedChanges = selection._tag === "ChangesSection";

	useNavigationIndexHotkeys({
		focusedPanel,
		navigationIndex,
		projectId,
		group: "Outline",
		panel: "outline",
		select,
		selection,
	});

	useHotkeys([
		{
			hotkey: outlineHotkeys.selectBranch.hotkey,
			callback: openBranchPicker,
			options: {
				conflictBehavior: "allow",
				meta: outlineHotkeys.selectBranch.meta,
			},
		},
		{
			hotkey: outlineHotkeys.selectChanges.hotkey,
			callback: () => {
				dispatch(projectActions.selectOutline({ projectId, selection: changesSectionOperand }));
				focusPanel("outline");
			},
			options: { conflictBehavior: "allow", meta: outlineHotkeys.selectChanges.meta },
		},
		{
			hotkey: outlineHotkeys.composeCommitMessage.hotkey,
			callback: () => {
				dispatch(projectActions.selectOutline({ projectId, selection: changesSectionOperand }));
				focusCommitMessageInput();
			},
			options: {
				conflictBehavior: "allow",
				meta: outlineHotkeys.composeCommitMessage.meta,
			},
		},
		...Match.value(selection).pipe(
			Match.tag(
				"Commit",
				(selection): UseHotkeyDefinition => ({
					hotkey: outlineHotkeys.rewordCommit.hotkey,
					callback: () => {
						dispatch(projectActions.startRewordCommit({ projectId, commit: selection }));
					},
					options: {
						conflictBehavior: "allow",
						enabled: defaultOutlineHotkeysEnabled,
						meta: outlineHotkeys.rewordCommit.meta,
					},
				}),
			),
			Match.tag(
				"Branch",
				(selection): UseHotkeyDefinition => ({
					hotkey: outlineHotkeys.renameBranch.hotkey,
					callback: () => {
						dispatch(projectActions.startRenameBranch({ projectId, branch: selection }));
					},
					options: {
						conflictBehavior: "allow",
						enabled: defaultOutlineHotkeysEnabled,
						meta: outlineHotkeys.renameBranch.meta,
					},
				}),
			),
			Match.tag(
				"ChangesSection",
				(): UseHotkeyDefinition => ({
					hotkey: outlineHotkeys.editChangesCommitMessage.hotkey,
					callback: focusCommitMessageInput,
					options: {
						conflictBehavior: "allow",
						enabled: defaultOutlineHotkeysEnabled,
						meta: outlineHotkeys.editChangesCommitMessage.meta,
					},
				}),
			),
			Match.orElse(() => null),
			(x) => (x ? [x] : []),
		),
		{
			hotkey: outlineHotkeys.amendCommit.hotkey,
			callback: amendCommit,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit,
				meta: outlineHotkeys.amendCommit.meta,
			},
		},
		{
			hotkey: outlineHotkeys.moveCommitUp.hotkey,
			callback: () => moveSelectedCommit(-1),
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !commitMoveMutation.isPending,
				meta: outlineHotkeys.moveCommitUp.meta,
			},
		},
		{
			hotkey: outlineHotkeys.moveCommitDown.hotkey,
			callback: () => moveSelectedCommit(1),
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !commitMoveMutation.isPending,
				meta: outlineHotkeys.moveCommitDown.meta,
			},
		},
		...Match.value(selection).pipe(
			Match.tags({
				Commit: (selection): RelativeTo => ({ type: "commit", subject: selection.commitId }),
				Branch: (selection): RelativeTo => ({
					type: "referenceBytes",
					subject: selection.branchRef,
				}),
			}),
			Match.orElse(() => null),
			(relativeTo) =>
				relativeTo
					? [
							{
								hotkey: outlineHotkeys.composeCommitHere.hotkey,
								callback: () => composeCommitHere(relativeTo),
								options: {
									conflictBehavior: "allow",
									enabled: defaultOutlineHotkeysEnabled,
									meta: outlineHotkeys.composeCommitHere.meta,
								},
							} satisfies UseHotkeyDefinition,
						]
					: [],
		),
		{
			hotkey: outlineHotkeys.absorb.hotkey,
			callback: () => {
				enterAbsorbMode(changesSectionOperand, { type: "all" });
			},
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultOutlineHotkeysEnabled &&
					isSelectedChanges &&
					worktreeChanges &&
					worktreeChanges.changes.length > 0,
				meta: outlineHotkeys.absorb.meta,
			},
		},
		{
			hotkey: outlineHotkeys.redo.hotkey,
			callback: () => restoreSnapshotMutation.mutate("redo"),
			options: {
				enabled: defaultOutlineHotkeysEnabled && !restoreSnapshotMutation.isPending,
				meta: outlineHotkeys.redo.meta,
				ignoreInputs: true,
			},
		},
		{
			hotkey: outlineHotkeys.undo.hotkey,
			callback: () => restoreSnapshotMutation.mutate("undo"),
			options: {
				enabled: defaultOutlineHotkeysEnabled && !restoreSnapshotMutation.isPending,
				meta: outlineHotkeys.undo.meta,
				ignoreInputs: true,
			},
		},
	]);
};

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

export const OutlinePanel: FC<PanelProps> = ({ ...panelProps }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const selection = useAppSelector((state) => selectProjectSelectionOutline(state, projectId));

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

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

	const navigationIndex = useNavigationIndex({
		projectId,
		absorptionTargetKeys,
	});

	const dryRunOperation = Match.value(outlineMode).pipe(
		Match.tag(
			"Transfer",
			({ value: mode }) => getTransferOperation({ mode, target: selection }) ?? undefined,
		),
		Match.orElse(() => undefined),
	);

	// TODO: debounce?
	const dryRunOperationQuery = useDryRunOperation({ projectId, operation: dryRunOperation });
	const dryRunWorkspace = dryRunOperationQuery.data?.workspace ?? null;

	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));

	useOutlineTreeHotkeys({
		navigationIndex,
		projectId,
	});

	const operationSource = getOperationSource(outlineMode);

	const commitTargetState = useAppSelector((state) => selectProjectCommitTarget(state, projectId));
	const targetComboboxItems = buildCommitTargetComboboxItems({ headInfo, commitTargetState });
	const commitTarget = selectCommitTargetComboboxItem({
		items: targetComboboxItems,
		commitTargetState,
	});

	const dispatch = useAppDispatch();
	const openApplyBranchPicker = () => {
		dispatch(projectActions.openApplyBranchPicker({ projectId }));
	};

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	return (
		<NavigationIndexContext value={navigationIndex}>
			<AbsorptionTargetKeysContext value={absorptionTargetKeys}>
				<DryRunWorkspaceContext value={dryRunWorkspace}>
					<Panel
						{...panelProps}
						tabIndex={0}
						role="tree"
						aria-activedescendant={treeItemId(selection)}
						className={classes(panelProps.className, styles.panel)}
					>
						<header className={styles.workspaceControls}>
							<div className={styles.workspaceControlsLeft}>
								<h1 className={styles.workspaceName}>{selectedProject.title}</h1>
								<ActivitySpinner />
							</div>

							<ShortcutButton
								hotkey={workspaceHotkeys.applyBranch.hotkey}
								hotkeyOptions={{ meta: workspaceHotkeys.applyBranch.meta }}
								onClick={openApplyBranchPicker}
							>
								Apply branch
							</ShortcutButton>
						</header>

						<Changes
							projectId={projectId}
							commitTarget={commitTarget}
							targetComboboxItems={targetComboboxItems}
						/>

						<div className={styles.headInfo}>
							{headInfo?.stacks.map((stack) => (
								<StackC
									key={stack.id}
									projectId={projectId}
									stack={stack}
									commitTarget={commitTarget?.relativeTo ?? null}
								/>
							))}
						</div>

						{operationSource && headInfo && (
							<div className={styles.operationSourcePreview}>
								<OperationSourceLabel headInfo={headInfo} source={operationSource} />
								{outlineMode._tag === "Absorb" && absorptionPlanQuery?.isPending && (
									<Icon name="spinner" aria-label="Loading absorb plan" />
								)}
							</div>
						)}
					</Panel>
				</DryRunWorkspaceContext>
			</AbsorptionTargetKeysContext>
		</NavigationIndexContext>
	);
};

const useIsSelected = ({ projectId, operand }: { projectId: string; operand: Operand }): boolean =>
	useAppSelector((state) => {
		const selection = selectProjectSelectionOutline(state, projectId);

		return operandEquals(selection, operand);
	});

const treeItemId = (operand: Operand): string =>
	`outline-treeitem-${encodeURIComponent(operandIdentityKey(operand))}`;

const ItemRowMenuButton: FC<{
	ariaLabel: string;
	menuItems: Array<NativeMenuItem>;
	disabled?: boolean;
	isSelected?: boolean;
}> = ({ ariaLabel, menuItems, disabled = false, isSelected = false }) => (
	<Toolbar.Button
		aria-label={ariaLabel}
		disabled={disabled === true}
		render={<Button variant={isSelected === true ? "inverted" : "ghost"} size="small" />}
		onClick={(event) => {
			void showNativeMenuFromTrigger(event.currentTarget, menuItems);
		}}
	>
		<Icon name="kebab" />
	</Toolbar.Button>
);

const CommitTargetIndicator: FC = () => {
	const [open, setOpen] = useState(false);

	return (
		<Tooltip
			open={open}
			// [ref:tooltip-disable-hoverable-popup]
			disableHoverablePopup
			trigger={
				<span
					className={styles.commitTargetIndicator}
					aria-label="Commit target"
					onMouseEnter={() => setOpen(true)}
					onMouseLeave={() => setOpen(false)}
				>
					<Icon name="bullseye" />
				</span>
			}
			content="Commit target"
			positionerProps={{ sideOffset: 8 }}
		/>
	);
};

const ItemRow: FC<
	{
		projectId: string;
		operand: Operand;
	} & Omit<ComponentProps<typeof WorkspaceItemRow>, "inert" | "isSelected" | "onSelect">
> = ({ projectId, operand, ...props }) => {
	const dispatch = useAppDispatch();
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useIsSelected({ projectId, operand });
	const selectItem = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
	};

	return (
		<WorkspaceItemRow
			{...props}
			inert={!navigationIndexIncludes(navigationIndex, operand)}
			isSelected={isSelected}
			onSelect={selectItem}
		/>
	);
};

const TreeItem: FC<
	{
		projectId: string;
		operand: Operand;
		expanded?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, expanded, render, ...props }) => {
	const isSelected = useIsSelected({ projectId, operand });

	return useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(operand),
			role: "treeitem",
			"aria-selected": isSelected,
			"aria-expanded": expanded,
		}),
	});
};
const OperandC: FC<
	{
		projectId: string;
		operand: Operand;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, render, ...props }) => {
	const isSelected = useIsSelected({ projectId, operand });
	const absorptionTargetKeys = assert(use(AbsorptionTargetKeysContext));
	const isAbsorptionTarget = absorptionTargetKeys.has(operandIdentityKey(operand));
	const navigationIndex = assert(use(NavigationIndexContext));

	return useRender({
		render: (
			<OperationSourceC
				projectId={projectId}
				selectionScope="outline"
				source={operand}
				render={
					<OperationTarget
						enabled={navigationIndexIncludes(navigationIndex, operand)}
						projectId={projectId}
						target={operand}
						isSelected={isSelected}
						isAbsorptionTarget={isAbsorptionTarget}
						render={render}
					/>
				}
			/>
		),
		defaultTagName: "div",
		props,
	});
};

const EditorHelp: FC<{
	hotkeys: Array<{ hotkey: string; name: string }>;
}> = ({ hotkeys }) => (
	<div className={styles.editorHelp}>
		{hotkeys.map((hotkey, index) => (
			<Fragment key={hotkey.hotkey}>
				{index > 0 && " • "}
				<kbd className={styles.editorShortcut}>{formatForDisplay(hotkey.hotkey)}</kbd>
				<span className={styles.editorShortcutLabel}> to {hotkey.name}</span>
			</Fragment>
		))}
	</div>
);

const InlineRewordCommit: FC<{
	message: string;
	onSubmit: (value: string) => void;
	onExit: () => void;
	projectId: string;
}> = ({ message, onSubmit, onExit, projectId }) => {
	const formRef = useRef<HTMLFormElement | null>(null);
	const focusedPanel = useFocusedProjectPanel(projectId);
	const submitAction = (formData: FormData) => {
		onExit();
		onSubmit(formData.get("message") as string);
	};

	useHotkey("Enter", () => formRef.current?.requestSubmit(), {
		conflictBehavior: "allow",
		enabled: focusedPanel === "outline",
		ignoreInputs: false,
		meta: { group: "Reword commit", name: "Save reworded commit" },
	});

	useHotkey("Escape", onExit, {
		conflictBehavior: "allow",
		enabled: focusedPanel === "outline",
		ignoreInputs: false,
		meta: { group: "Reword commit", name: "Cancel reword commit" },
	});

	return (
		<form ref={formRef} className={styles.editorForm} action={submitAction}>
			<textarea
				ref={(el) => {
					if (!el) return;
					el.focus();
					const firstNewline = el.textContent.indexOf("\n");
					const cursorPosition = firstNewline !== -1 ? firstNewline : el.value.length;
					el.setSelectionRange(cursorPosition, cursorPosition);
				}}
				aria-label="Commit message"
				name="message"
				defaultValue={message.trim()}
				className={classes(styles.editorInput, styles.rewordCommitInput)}
			/>
			<EditorHelp
				hotkeys={[
					{ hotkey: "Enter", name: "Save" },
					{ hotkey: "Escape", name: "Cancel" },
				]}
			/>
		</form>
	);
};

const CommitRow: FC<
	{
		commit: Commit;
		projectId: string;
		stackId: string;
		isCommitTarget: boolean;
	} & ComponentProps<"div">
> = ({ commit, projectId, stackId, isCommitTarget, ...restProps }) => {
	const isHighlighted = useAppSelector((state) =>
		selectProjectHighlightedCommitIds(state, projectId).includes(commit.id),
	);
	const dryRunCommit = useDryRunCommit(commit.id);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const dispatch = useAppDispatch();
	const commitOperandV: CommitOperand = {
		stackId,
		commitId: commit.id,
	};
	const operand = commitOperand(commitOperandV);
	const isSelected = useIsSelected({ projectId, operand });
	const isRewording =
		isSelected &&
		outlineMode._tag === "RewordCommit" &&
		operandEquals(operand, commitOperand(outlineMode.operand));
	const [optimisticMessage, setOptimisticMessage] = useOptimistic(
		commit.message,
		(_currentMessage, nextMessage: string) => nextMessage,
	);
	const [isCommitMessagePending, startCommitMessageTransition] = useTransition();

	const commitWithOptimisticMessage: Commit = {
		...commit,
		message: optimisticMessage,
	};
	const { hasConflicts } = dryRunCommit ? dryRunCommit : commitWithOptimisticMessage;

	const commitInsertBlank = useMutation({
		mutationFn: window.lite.commitInsertBlank,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to insert commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
	const commitDiscard = useMutation({
		mutationFn: window.lite.commitDiscard,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to discard commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
	const commitReword = useMutation({
		mutationFn: window.lite.commitReword,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to reword commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});

	const insertBlankCommitAbove = () => {
		commitInsertBlank.mutate({
			projectId,
			relativeTo: { type: "commit", subject: commit.id },
			side: "above",
			dryRun: false,
		});
	};

	const insertBlankCommitBelow = () => {
		commitInsertBlank.mutate({
			projectId,
			relativeTo: { type: "commit", subject: commit.id },
			side: "below",
			dryRun: false,
		});
	};

	const deleteCommit = () => {
		commitDiscard.mutate({
			projectId,
			subjectCommitId: commit.id,
			dryRun: false,
		});
	};

	const cutCommit = () => {
		dispatch(
			projectActions.enterTransferMode({
				projectId,
				mode: keyboardTransferOperationMode({
					source: operand,
					operationType: "rub",
				}),
			}),
		);
	};

	const startEditing = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		dispatch(projectActions.startRewordCommit({ projectId, commit: commitOperandV }));
	};

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		focusPanel("outline");
	};

	const toastManager = Toast.useToastManager();

	const saveNewMessage = (newMessage: string) => {
		const initialMessage = commit.message.trim();
		const trimmed = newMessage.trim();
		if (trimmed === initialMessage) return;
		startCommitMessageTransition(async () => {
			setOptimisticMessage(trimmed);
			try {
				await commitReword.mutateAsync({
					projectId,
					commitId: commit.id,
					message: trimmed,
					dryRun: false,
				});
			} catch (error) {
				// oxlint-disable-next-line no-console
				console.error(error);

				toastManager.add({
					type: "error",
					title: "Failed to reword commit",
					description: errorMessageForToast(error),
					priority: "high",
				});
			}
		});
	};

	const amendCommit = () => {
		dispatch(
			projectActions.enterTransferMode({
				projectId,
				mode: keyboardTransferOperationMode({
					source: changesSectionOperand,
					operationType: "rub",
				}),
			}),
		);
		focusPanel("outline");
	};

	const relativeTo: RelativeTo = { type: "commit", subject: commit.id };

	const composeCommitHere = () => {
		dispatch(projectActions.setCommitTarget({ projectId, commitTarget: relativeTo }));
		focusCommitMessageInput();
	};

	const amendCommitContextMenuItem = nativeMenuItem({
		label: "Amend Commit",
		enabled: true,
		accelerator: toElectronAccelerator(outlineHotkeys.amendCommit.hotkey),
		onSelect: amendCommit,
	});
	const cutCommitContextMenuItem = nativeMenuItem({
		label: "Cut Commit",
		enabled: true,
		onSelect: cutCommit,
	});
	const startEditingContextMenuItem = nativeMenuItem({
		label: "Reword Commit",
		enabled: !isCommitMessagePending,
		accelerator: toElectronAccelerator(outlineHotkeys.rewordCommit.hotkey),
		onSelect: startEditing,
	});
	const insertBlankCommitAboveContextMenuItem = nativeMenuItem({
		label: "Above",
		enabled: true,
		onSelect: insertBlankCommitAbove,
	});
	const insertBlankCommitBelowContextMenuItem = nativeMenuItem({
		label: "Below",
		enabled: true,
		onSelect: insertBlankCommitBelow,
	});
	const deleteCommitContextMenuItem = nativeMenuItem({
		label: "Delete Commit",
		enabled: !commitDiscard.isPending,
		onSelect: deleteCommit,
	});
	const setCommitTargetContextMenuItem = nativeMenuItem({
		label: "Compose Commit Here",
		accelerator: toElectronAccelerator(outlineHotkeys.composeCommitHere.hotkey),
		onSelect: composeCommitHere,
	});
	const copyChangeIdContextMenuItem = nativeMenuItem({
		label: "Change ID",
		onSelect: () => window.lite.clipboardWriteText(commit.changeId),
	});
	const copyCommitIdContextMenuItem = nativeMenuItem({
		label: "Commit ID",
		onSelect: () => window.lite.clipboardWriteText(commit.id),
	});

	const menuItems: Array<NativeMenuItem> = [
		startEditingContextMenuItem,
		amendCommitContextMenuItem,
		cutCommitContextMenuItem,
		nativeMenuSeparator,
		setCommitTargetContextMenuItem,
		nativeMenuItem({
			label: "Copy",
			submenu: [copyChangeIdContextMenuItem, copyCommitIdContextMenuItem],
		}),
		nativeMenuItem({
			label: "Add Empty Commit",
			submenu: [insertBlankCommitAboveContextMenuItem, insertBlankCommitBelowContextMenuItem],
		}),
		nativeMenuSeparator,
		deleteCommitContextMenuItem,
	];

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			operand={operand}
			className={classes(
				restProps.className,
				isHighlighted && workspaceItemRowStyles.itemRowHighlighted,
			)}
		>
			{isRewording ? (
				<InlineRewordCommit
					message={optimisticMessage}
					onSubmit={saveNewMessage}
					onExit={endEditing}
					projectId={projectId}
				/>
			) : (
				<>
					<div className={styles.commitRowLabel}>
						<span
							className={styles.commitState}
							data-status={commitIsDiverged(commit) ? "Diverged" : commit.state.type}
						/>

						<div
							className={workspaceItemRowStyles.itemRowLabel}
							onDoubleClick={outlineMode._tag === "Default" ? startEditing : undefined}
							onContextMenu={
								outlineMode._tag === "Default"
									? (event) => {
											void showNativeContextMenu(event, menuItems);
										}
									: undefined
							}
						>
							{commitTitle(commitWithOptimisticMessage.message)}
							{hasConflicts && " ⚠️"}
						</div>
					</div>
					{outlineMode._tag === "Default" && (
						<WorkspaceItemRowToolbar aria-label="Commit actions">
							<ItemRowMenuButton
								ariaLabel="Commit menu"
								menuItems={menuItems}
								isSelected={isSelected}
							/>
						</WorkspaceItemRowToolbar>
					)}
					{isCommitTarget && <CommitTargetIndicator />}
				</>
			)}
		</ItemRow>
	);
};

const CommitC: FC<{
	commit: Commit;
	projectId: string;
	stackId: string;
	isCommitTarget: boolean;
}> = ({ commit, projectId, stackId, isCommitTarget }) => {
	const operand = commitOperand({ stackId, commitId: commit.id });

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={commitTitle(commit.message)}
			render={<OperandC projectId={projectId} operand={operand} />}
		>
			<CommitRow
				commit={commit}
				projectId={projectId}
				stackId={stackId}
				isCommitTarget={isCommitTarget}
			/>
		</TreeItem>
	);
};

const ChangesSectionRow: FC<{
	changes: Array<TreeChange>;

	projectId: string;
}> = ({ changes, projectId }) => {
	const operand = changesSectionOperand;
	const isSelected = useIsSelected({ projectId, operand });
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const dispatch = useAppDispatch();
	const enterAbsorbMode = (source: Operand, sourceTarget: AbsorptionTarget) => {
		dispatch(projectActions.enterAbsorbMode({ projectId, source, sourceTarget }));
	};

	const absorb = () => {
		enterAbsorbMode(operand, { type: "all" });
	};

	const absorbContextMenuItem = nativeMenuItem({
		label: "Absorb",
		enabled: changes.length > 0,
		accelerator: toElectronAccelerator(outlineHotkeys.absorb.hotkey),
		onSelect: absorb,
	});

	const menuItems: Array<NativeMenuItem> = [absorbContextMenuItem];

	return (
		<ItemRow projectId={projectId} operand={operand}>
			<div
				className={classes(
					workspaceItemRowStyles.itemRowLabel,
					styles.changesRowLabel,
					isSelected && styles.selected,
				)}
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
			>
				Changes
				<span className={styles.changesCountBubble}>{changes.length}</span>
			</div>
			{outlineMode._tag === "Default" && (
				<WorkspaceItemRowToolbar aria-label="Changes actions">
					<ItemRowMenuButton
						ariaLabel="Changes menu"
						menuItems={menuItems}
						isSelected={isSelected}
					/>
				</WorkspaceItemRowToolbar>
			)}
		</ItemRow>
	);
};

type CommitTargetComboboxItem = {
	label: string;
	relativeTo: RelativeTo;
};

const relativeToKey = (relativeTo: RelativeTo): string => {
	switch (relativeTo.type) {
		case "reference":
			return JSON.stringify(["reference", relativeTo.subject]);
		case "referenceBytes":
			return JSON.stringify(["referenceBytes", relativeTo.subject]);
		case "commit":
			return JSON.stringify(["commit", relativeTo.subject]);
	}
};

const relativeToEquals = (a: RelativeTo, b: RelativeTo): boolean =>
	relativeToKey(a) === relativeToKey(b);

const buildCommitTargetComboboxItems = ({
	headInfo,
	commitTargetState,
}: {
	headInfo: RefInfo | undefined;
	commitTargetState: RelativeTo | null;
}): Array<CommitTargetComboboxItem> => {
	const commitTarget =
		headInfo && commitTargetState?.type === "commit"
			? findCommit({ headInfo, commitId: commitTargetState.subject })
			: null;

	return [
		...(commitTarget
			? ([
					{
						label: `Commit: ${commitTitle(commitTarget.message)}`,
						relativeTo: { type: "commit", subject: commitTarget.id },
					},
				] satisfies Array<CommitTargetComboboxItem>)
			: []),
		...(headInfo
			? headInfo.stacks.flatMap(
					(stack): Array<CommitTargetComboboxItem> =>
						stack.segments.flatMap((segment): Array<CommitTargetComboboxItem> => {
							const refName = segment.refName;
							if (!refName) return [];

							return [
								{
									label: refName.displayName,
									relativeTo: { type: "referenceBytes", subject: refName.fullNameBytes },
								},
							];
						}),
				)
			: []),
	];
};

const selectCommitTargetComboboxItem = ({
	items,
	commitTargetState,
}: {
	items: Array<CommitTargetComboboxItem>;
	commitTargetState: RelativeTo | null;
}): CommitTargetComboboxItem | null =>
	(commitTargetState &&
		items.find((item) => relativeToEquals(item.relativeTo, commitTargetState))) ??
	items[0] ??
	null;

const CommitTargetComboboxPopup: FC = () => (
	<Combobox.Popup className={classes(uiStyles.popup, styles.commitTargetComboboxPopup)}>
		<Combobox.Input
			aria-label="Search targets"
			placeholder="Search targets…"
			className={styles.commitTargetComboboxInput}
		/>
		<Combobox.Empty>
			<div className={styles.commitTargetComboboxEmpty}>No targets found.</div>
		</Combobox.Empty>
		<Combobox.List className={styles.commitTargetComboboxList}>
			{(item: CommitTargetComboboxItem) => (
				<Combobox.Item
					key={relativeToKey(item.relativeTo)}
					value={item}
					className={styles.commitTargetComboboxItem}
				>
					{item.label}
				</Combobox.Item>
			)}
		</Combobox.List>
	</Combobox.Popup>
);

const commitMessageInputId = "commit-message-input";
const focusCommitMessageInput = () => {
	document.getElementById(commitMessageInputId)?.focus();
};

const Changes: FC<{
	projectId: string;
	commitTarget: CommitTargetComboboxItem | null;
	targetComboboxItems: Array<CommitTargetComboboxItem>;
}> = ({ projectId, commitTarget, targetComboboxItems }) => {
	const toastManager = Toast.useToastManager();
	const queryClient = useQueryClient();
	const dispatch = useAppDispatch();

	const commitCreate = useMutation({
		mutationFn: async () => {
			if (!commitTarget) throw new Error("No target.");

			const worktreeChanges = await queryClient.fetchQuery(
				changesInWorktreeQueryOptions(projectId),
			);
			const changes = worktreeChanges.changes.map((change) => createDiffSpec(change, []));

			return await window.lite.commitCreate({
				projectId,
				relativeTo: commitTarget.relativeTo,
				changes,
				side: Match.value(commitTarget.relativeTo).pipe(
					Match.withReturnType<InsertSide>(),
					Match.when({ type: "commit" }, () => "above"),
					Match.when({ type: "reference" }, () => "below"),
					Match.when({ type: "referenceBytes" }, () => "below"),
					Match.exhaustive,
				),
				message: commitTextareaRef.current?.value ?? "",
				dryRun: false,
			});
		},
		onSuccess: async (response) => {
			if (commitTarget?.relativeTo.type === "commit" && response.newCommit !== null)
				dispatch(
					projectActions.setCommitTarget({
						projectId,
						commitTarget: { type: "commit", subject: response.newCommit },
					}),
				);

			if (response.rejectedChanges.length > 0)
				toastManager.add(
					rejectedChangesToastOptions({
						newCommit: response.newCommit,
						rejectedChanges: response.rejectedChanges,
					}),
				);

			if (response.newCommit !== null && commitTextareaRef.current)
				commitTextareaRef.current.value = "";
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
	const commitAmend = useMutation({
		mutationFn: async () => {
			if (!commitTarget) throw new Error("No target.");

			const headInfo = await queryClient.fetchQuery(headInfoQueryOptions(projectId));

			const commitId = resolveRelativeTo({
				headInfo,
				relativeTo: commitTarget.relativeTo,
			});
			if (commitId === null) throw new Error("No commit to amend.");

			const worktreeChanges = await queryClient.fetchQuery(
				changesInWorktreeQueryOptions(projectId),
			);
			const changes = worktreeChanges.changes.map((change) => createDiffSpec(change, []));

			return await window.lite.commitAmend({
				projectId,
				commitId,
				changes,
				dryRun: false,
			});
		},
		onSuccess: async (response, _input, _ctx, { client }) => {
			client.setQueryData(headInfoQueryOptions(projectId).queryKey, response.workspace.headInfo);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);

			if (commitTarget?.relativeTo.type === "commit" && response.newCommit !== null)
				dispatch(
					projectActions.setCommitTarget({
						projectId,
						commitTarget: { type: "commit", subject: response.newCommit },
					}),
				);

			if (response.rejectedChanges.length > 0)
				toastManager.add(
					rejectedChangesToastOptions({
						newCommit: response.newCommit,
						rejectedChanges: response.rejectedChanges,
					}),
				);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to amend commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});

	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const operand = changesSectionOperand;
	const commitTextareaRef = useRef<HTMLTextAreaElement | null>(null);

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const isAltHeld = useKeyHold("Alt");
	const isAmendMode = isAltHeld;
	const isCommitOrAmendPending = commitCreate.isPending || commitAmend.isPending;
	const canCommit =
		outlineMode._tag === "Default" && commitTarget !== null && !isCommitOrAmendPending;
	const canAmend =
		canCommit &&
		worktreeChanges &&
		worktreeChanges.changes.length > 0 &&
		headInfo &&
		resolveRelativeTo({ headInfo, relativeTo: commitTarget.relativeTo }) !== null;
	const canCommitOrAmend = isAmendMode ? canAmend : canCommit;

	const [open, setOpen] = useState(false);

	const selectBranch = (option: CommitTargetComboboxItem | null) => {
		dispatch(
			projectActions.setCommitTarget({
				projectId,
				commitTarget: option?.relativeTo ?? null,
			}),
		);
		setOpen(false);
	};

	const selectChanges = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
	};
	const submit: SubmitEventHandler = (event) => {
		event.preventDefault();

		if (isAmendMode) {
			commitAmend.mutate();
			return;
		}

		commitCreate.mutate();
	};
	const commitMenuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: "Commit",
			enabled: canCommit,
			accelerator: toElectronAccelerator(changesHotkeys.commit.hotkey),
			onSelect: () => commitCreate.mutate(),
		}),
		nativeMenuItem({
			label: "Amend Commit",
			enabled: canAmend,
			accelerator: toElectronAccelerator(changesHotkeys.amendCommit.hotkey),
			onSelect: () => commitAmend.mutate(),
		}),
	];

	useHotkeys([
		{
			hotkey: changesHotkeys.selectCommitBranch.hotkey,
			callback: () => setOpen(true),
			options: {
				conflictBehavior: "allow",
				enabled: outlineMode._tag === "Default" && !isCommitOrAmendPending,
				meta: changesHotkeys.selectCommitBranch.meta,
			},
		},
		{
			hotkey: changesHotkeys.commit.hotkey,
			callback: () => commitCreate.mutate(),
			options: {
				conflictBehavior: "allow",
				enabled: canCommit,
				ignoreInputs: false,
				meta: changesHotkeys.commit.meta,
			},
		},
		{
			hotkey: changesHotkeys.amendCommit.hotkey,
			callback: () => commitAmend.mutate(),
			options: {
				conflictBehavior: "allow",
				enabled: canAmend,
				ignoreInputs: false,
				meta: changesHotkeys.amendCommit.meta,
			},
		},
	]);

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={`Changes (${worktreeChanges?.changes.length ?? 0})`}
			className={classes(workspaceItemRowStyles.section, styles.changesSection)}
			render={
				<OperandC projectId={projectId} operand={operand} render={<form onSubmit={submit} />} />
			}
		>
			<ChangesSectionRow changes={worktreeChanges?.changes ?? []} projectId={projectId} />

			<div className={styles.commitControls}>
				<textarea
					id={commitMessageInputId}
					ref={commitTextareaRef}
					aria-label="Compose commit message"
					disabled={outlineMode._tag !== "Default"}
					readOnly={isCommitOrAmendPending}
					placeholder="Commit message..."
					className={styles.commitTextarea}
					onFocus={selectChanges}
					onKeyDown={(event) => {
						if (event.key !== "Escape") return;
						event.preventDefault();
						focusPanel("outline");
					}}
				/>

				<div className={styles.commitControlsFooter}>
					<Combobox.Root<CommitTargetComboboxItem>
						items={targetComboboxItems}
						open={open}
						onOpenChange={setOpen}
						// Note `undefined` means uncontrolled.
						value={commitTarget ?? null}
						onValueChange={selectBranch}
						itemToStringLabel={(x) => x.label}
						itemToStringValue={(x) => relativeToKey(x.relativeTo)}
						isItemEqualToValue={(a, b) => relativeToEquals(a.relativeTo, b.relativeTo)}
						autoHighlight
						disabled={outlineMode._tag !== "Default" || isCommitOrAmendPending}
					>
						<Combobox.Trigger
							className={classes(styles.commitTargetComboboxTrigger)}
							aria-label="Select branch"
							render={
								<ShortcutButton
									hotkey={changesHotkeys.selectCommitBranch.hotkey}
									hotkeyOptions={{ meta: changesHotkeys.selectCommitBranch.meta }}
								/>
							}
						>
							<Combobox.Value placeholder="Select branch" />
						</Combobox.Trigger>
						<Combobox.Portal>
							<Combobox.Positioner align="start" sideOffset={8}>
								<CommitTargetComboboxPopup />
							</Combobox.Positioner>
						</Combobox.Portal>
					</Combobox.Root>

					<DropdownButton
						hotkey={isAmendMode ? changesHotkeys.amendCommit.hotkey : changesHotkeys.commit.hotkey}
						hotkeyOptions={{
							meta: isAmendMode ? changesHotkeys.amendCommit.meta : changesHotkeys.commit.meta,
						}}
						type="submit"
						disabled={!canCommitOrAmend || outlineMode._tag !== "Default" || isCommitOrAmendPending}
						onMenuOpen={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, commitMenuItems);
						}}
						menuAriaLabel="Commit options"
					>
						{isAmendMode ? "Amend" : "Commit"}
					</DropdownButton>
				</div>
			</div>
		</TreeItem>
	);
};

const InlineRenameBranch: FC<{
	branchName: string;
	onSubmit: (value: string) => void;
	onExit: () => void;
	projectId: string;
}> = ({ branchName, onSubmit, onExit, projectId }) => {
	const formRef = useRef<HTMLFormElement | null>(null);
	const focusedPanel = useFocusedProjectPanel(projectId);
	const submitAction = (formData: FormData) => {
		onExit();
		onSubmit(formData.get("branchName") as string);
	};

	useHotkey("Enter", () => formRef.current?.requestSubmit(), {
		conflictBehavior: "allow",
		enabled: focusedPanel === "outline",
		ignoreInputs: false,
		meta: { group: "Rename branch", name: "Save branch name" },
	});

	useHotkey("Escape", onExit, {
		conflictBehavior: "allow",
		enabled: focusedPanel === "outline",
		ignoreInputs: false,
		meta: { group: "Rename branch", name: "Cancel branch rename" },
	});

	return (
		<form ref={formRef} className={styles.editorForm} action={submitAction}>
			<input
				aria-label="Branch name"
				ref={(el) => {
					if (!el) return;
					el.focus();
					el.select();
				}}
				name="branchName"
				defaultValue={branchName}
				className={classes(styles.editorInput, styles.renameBranchInput)}
			/>
			<EditorHelp
				hotkeys={[
					{ hotkey: "Enter", name: "Save" },
					{ hotkey: "Escape", name: "Cancel" },
				]}
			/>
		</form>
	);
};

const BranchRow: FC<
	{
		projectId: string;
		branchName: string;
		branchRef: Array<number>;
		stackId: string;
		isCommitTarget: boolean;
		canTearOffBranch: boolean;
		canRemoveBranch: boolean;
	} & ComponentProps<"div">
> = ({
	projectId,
	branchName,
	branchRef,
	stackId,
	isCommitTarget,
	canTearOffBranch,
	canRemoveBranch,
	...restProps
}) => {
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const dispatch = useAppDispatch();
	const branchOperandV: BranchOperand = {
		stackId,
		branchRef,
	};
	const operand = branchOperand(branchOperandV);
	const isSelected = useIsSelected({ projectId, operand });
	const isRenaming =
		outlineMode._tag === "RenameBranch" &&
		operandEquals(operand, branchOperand(outlineMode.operand));
	const [optimisticBranchName, setOptimisticBranchName] = useOptimistic(
		branchName,
		(_currentBranchName, nextBranchName: string) => nextBranchName,
	);
	const [isRenamePending, startRenameTransition] = useTransition();

	const updateBranchName = useMutation({
		mutationFn: window.lite.updateBranchName,
		onSuccess: async (_response, input, _context, mutation) => {
			const newBranchRef = encodeRefName(`refs/heads/${input.newName}`);
			const newBranch: BranchOperand = {
				stackId,
				// TODO: ideally the API would return the new ref?
				branchRef: newBranchRef,
			};

			mutation.client.setQueryData(headInfoQueryOptions(projectId).queryKey, (headInfo) => {
				if (!headInfo) return headInfo;

				return renameBranchInHeadInfo({
					headInfo,
					stackId,
					branchRef,
					newName: input.newName,
					newBranchRef,
				});
			});

			dispatch(
				projectActions.updateRewrittenBranchReferences({
					projectId,
					oldBranch: branchOperandV,
					newBranch,
				}),
			);
			dispatch(projectActions.exitMode({ projectId }));
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to rename branch",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});

	const startEditing = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		dispatch(projectActions.startRenameBranch({ projectId, branch: branchOperandV }));
	};

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		focusPanel("outline");
	};

	const toastManager = Toast.useToastManager();

	const tearOffBranchMutation = useMutation({
		mutationFn: window.lite.tearOffBranch,
		onSuccess: async (response, _input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to tear off branch",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});

	// TODO: This mutation doesn't trigger any watcher events, hence the manual invalidation.
	const removeBranchMutation = useMutation({
		mutationFn: window.lite.removeBranch,
		onSuccess: async (_response, input, _context, mutation) => {
			await mutation.client.invalidateQueries({
				queryKey: headInfoQueryOptions(input.projectId).queryKey,
			});
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to remove branch",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});

	const saveBranchName = (newBranchName: string) => {
		const trimmed = newBranchName.trim();
		if (trimmed === "" || trimmed === branchName) return;
		startRenameTransition(async () => {
			setOptimisticBranchName(trimmed);
			try {
				await updateBranchName.mutateAsync({
					projectId,
					stackId,
					branchName,
					newName: trimmed,
				});
			} catch (error) {
				// oxlint-disable-next-line no-console
				console.error(error);

				toastManager.add({
					type: "error",
					title: "Failed to rename branch",
					description: errorMessageForToast(error),
					priority: "high",
				});
			}
		});
	};

	const relativeTo: RelativeTo = { type: "referenceBytes", subject: branchRef };

	const composeCommitHere = () => {
		dispatch(projectActions.setCommitTarget({ projectId, commitTarget: relativeTo }));
		focusCommitMessageInput();
	};

	const tearOffBranch = () => {
		tearOffBranchMutation.mutate({
			projectId,
			subjectBranch: decodeRefName(branchRef),
			dryRun: false,
		});
	};

	const startEditingContextMenuItem = nativeMenuItem({
		label: "Rename Branch",
		enabled: !isRenamePending,
		accelerator: toElectronAccelerator(outlineHotkeys.renameBranch.hotkey),
		onSelect: startEditing,
	});
	const setCommitTargetContextMenuItem = nativeMenuItem({
		label: "Compose Commit Here",
		accelerator: toElectronAccelerator(outlineHotkeys.composeCommitHere.hotkey),
		onSelect: composeCommitHere,
	});
	const copyBranchNameContextMenuItem = nativeMenuItem({
		label: "Copy Branch Name",
		onSelect: () => window.lite.clipboardWriteText(optimisticBranchName),
	});
	const tearOffBranchContextMenuItem = nativeMenuItem({
		label: "Tear Off Branch",
		enabled: canTearOffBranch && !tearOffBranchMutation.isPending,
		onSelect: tearOffBranch,
	});
	const removeBranchContextMenuItem = nativeMenuItem({
		label: "Remove Branch",
		enabled: canRemoveBranch,
		onSelect: () =>
			removeBranchMutation.mutate({
				projectId,
				stackId,
				branchName,
			}),
	});

	const menuItems: Array<NativeMenuItem> = [
		startEditingContextMenuItem,
		copyBranchNameContextMenuItem,
		setCommitTargetContextMenuItem,
		nativeMenuSeparator,
		tearOffBranchContextMenuItem,
		removeBranchContextMenuItem,
	];

	return (
		<ItemRow {...restProps} projectId={projectId} operand={operand}>
			{isRenaming ? (
				<InlineRenameBranch
					branchName={optimisticBranchName}
					onSubmit={saveBranchName}
					onExit={endEditing}
					projectId={projectId}
				/>
			) : (
				<>
					<div
						className={classes(workspaceItemRowStyles.itemRowLabel, styles.branchRowLabel)}
						onDoubleClick={outlineMode._tag === "Default" ? startEditing : undefined}
						onContextMenu={
							outlineMode._tag === "Default"
								? (event) => {
										void showNativeContextMenu(event, menuItems);
									}
								: undefined
						}
					>
						{optimisticBranchName}
					</div>
					{outlineMode._tag === "Default" && (
						<WorkspaceItemRowToolbar aria-label="Branch actions">
							<Toolbar.Button
								render={
									<Button
										variant={isSelected ? "inverted" : "ghost"}
										size="small"
										disabled
										aria-label="Push branch"
									/>
								}
							>
								<Icon name="arrow-line-up" />
							</Toolbar.Button>
							<ItemRowMenuButton
								ariaLabel="Branch menu"
								menuItems={menuItems}
								isSelected={isSelected}
							/>
						</WorkspaceItemRowToolbar>
					)}
					{isCommitTarget && <CommitTargetIndicator />}
				</>
			)}
		</ItemRow>
	);
};

const StackRow: FC<
	{
		projectId: string;
		stackId: string;
	} & ComponentProps<"div">
> = ({ projectId, stackId, ...restProps }) => {
	const operand = stackOperand({ stackId });
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const toastManager = Toast.useToastManager();

	const unapplyStack = useMutation({
		mutationFn: window.lite.unapplyStack,
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to unapply stack",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
	const unapply = () => {
		unapplyStack.mutate({ projectId, stackId });
	};

	const unapplyContextMenuItem = nativeMenuItem({
		label: "Unapply Stack",
		enabled: !unapplyStack.isPending,
		onSelect: unapply,
	});

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({ label: "Move Up", enabled: false }),
		nativeMenuItem({ label: "Move Down", enabled: false }),
		nativeMenuSeparator,
		unapplyContextMenuItem,
	];

	return (
		<ItemRow {...restProps} projectId={projectId} operand={operand} forceVisibleToolbar>
			<div
				className={classes(workspaceItemRowStyles.itemRowLabel, styles.stackRowLabel)}
				onContextMenu={
					outlineMode._tag === "Default"
						? (event) => {
								void showNativeContextMenu(event, menuItems);
							}
						: undefined
				}
			/>

			{outlineMode._tag === "Default" && (
				<WorkspaceItemRowToolbar aria-label="Stack actions">
					<ItemRowMenuButton ariaLabel="Stack menu" menuItems={menuItems} />
				</WorkspaceItemRowToolbar>
			)}
		</ItemRow>
	);
};

const BranchSegment: FC<{
	projectId: string;
	segment: Segment;
	refName: BranchReference;
	stackId: string;
	commitTarget: RelativeTo | null;
	canTearOffBranch: boolean;
	canRemoveBranch: boolean;
}> = ({
	projectId,
	segment,
	refName,
	stackId,
	commitTarget,
	canTearOffBranch,
	canRemoveBranch,
}) => {
	const operand = branchOperand({ stackId, branchRef: refName.fullNameBytes });

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={refName.displayName}
			expanded
			className={classes(workspaceItemRowStyles.section, styles.segment)}
		>
			<OperandC
				projectId={projectId}
				operand={operand}
				render={
					<BranchRow
						projectId={projectId}
						branchName={refName.displayName}
						branchRef={refName.fullNameBytes}
						stackId={stackId}
						canTearOffBranch={canTearOffBranch}
						canRemoveBranch={canRemoveBranch}
						isCommitTarget={
							commitTarget
								? relativeToEquals(commitTarget, {
										type: "referenceBytes",
										subject: refName.fullNameBytes,
									})
								: false
						}
					/>
				}
			/>

			{segment.commits.length === 0 ? (
				<div className={classes(workspaceItemRowStyles.itemRowEmpty, styles.noCommits)}>
					No commits.
				</div>
			) : (
				<div role="group">
					{segment.commits.map((commit) => (
						<CommitC
							key={commit.id}
							commit={commit}
							projectId={projectId}
							stackId={stackId}
							isCommitTarget={
								commitTarget
									? relativeToEquals(commitTarget, { type: "commit", subject: commit.id })
									: false
							}
						/>
					))}
				</div>
			)}
		</TreeItem>
	);
};

const BranchlessSegment: FC<{
	projectId: string;
	segment: Segment;
	stackId: string;
	commitTarget: RelativeTo | null;
}> = ({ projectId, segment, stackId, commitTarget }) => (
	<div className={classes(workspaceItemRowStyles.section, styles.segment)}>
		{segment.commits.map((commit) => (
			<CommitC
				key={commit.id}
				commit={commit}
				projectId={projectId}
				stackId={stackId}
				isCommitTarget={
					commitTarget
						? relativeToEquals(commitTarget, { type: "commit", subject: commit.id })
						: false
				}
			/>
		))}
	</div>
);

const StackC: FC<{
	projectId: string;
	stack: Stack;
	commitTarget: RelativeTo | null;
}> = ({ projectId, stack, commitTarget }) => {
	// From Caleb:
	// > There shouldn't be a way within GitButler to end up with a stack without a
	//   StackId. Users can disrupt our matching against our metadata by playing
	//   with references, but we currently also try to patch it up at certain points
	//   so it probably isn't too common.
	// For now we'll treat this as non-nullable until we identify cases where it
	// could genuinely be null (assuming backend correctness).
	// oxlint-disable-next-line typescript/no-non-null-assertion -- [tag:stack-id-required]
	const stackId = stack.id!;
	const operand = stackOperand({ stackId });
	const canTearOffBranch = stack.segments.length > 1;

	const hasAnyCommits = stack.segments.some((segment) => segment.commits.length > 0);
	const numBranches = stack.segments.filter((segment) => segment.refName !== null).length;
	const canRemoveBranches = !hasAnyCommits || numBranches > 1;

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label="Stack"
			expanded
			className={classes(styles.stack, workspaceItemRowStyles.section)}
			render={<OperandC projectId={projectId} operand={operand} />}
		>
			<StackRow projectId={projectId} stackId={stackId} />

			<div role="group" className={styles.segments}>
				{stack.segments.map((segment) =>
					segment.refName ? (
						<BranchSegment
							key={JSON.stringify(segment.refName.fullNameBytes)}
							projectId={projectId}
							segment={segment}
							refName={segment.refName}
							stackId={stackId}
							commitTarget={commitTarget}
							canTearOffBranch={canTearOffBranch}
							canRemoveBranch={canRemoveBranches}
						/>
					) : (
						// A segment should always either have a branch reference or at
						// least one commit.
						isNonEmptyArray(segment.commits) && (
							<BranchlessSegment
								key={segment.commits[0].id}
								projectId={projectId}
								segment={segment}
								stackId={stackId}
								commitTarget={commitTarget}
							/>
						)
					),
				)}
			</div>
		</TreeItem>
	);
};
