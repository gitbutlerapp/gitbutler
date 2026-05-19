import { headInfoQueryOptions, initialBranchIntegrationQueryOptions } from "#ui/api/queries.ts";
import { findBranchByRef } from "#ui/api/ref-info.ts";
import { encodeRefName } from "#ui/api/ref-name.ts";
import { branchOperand } from "#ui/operands.ts";
import { projectActions } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { CommitGraph } from "#ui/ui/CommitGraph/CommitGraph.tsx";
import type { CommitGraphRow } from "#ui/ui/CommitGraph/commitGraphRows.ts";
import { classes } from "#ui/ui/classes.ts";
import uiStyles from "#ui/ui/ui.module.css";
import { errorMessageForToast } from "#ui/errors.ts";
import { Toast } from "@base-ui/react";
import type { InitialBranchIntegration } from "@gitbutler/but-sdk";
import type { InteractiveIntegrationStep } from "@gitbutler/but-sdk";
import {
	draggable,
	dropTargetForElements,
} from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createRoute, useNavigate, useParams, useSearch } from "@tanstack/react-router";
import { FC, useEffect, useEffectEvent, useRef, useState } from "react";
import {
	buildCommitPickerOptions,
	buildIntegrationStepDrafts,
	buildInteractiveIntegration,
	changeIntegrationStepDraftKind,
	createDefaultIntegrationStepDraft,
	type CommitPickerOption,
	type IntegrationStepDraft,
	reorderIntegrationStepDrafts,
	updateIntegrationStepDraftCommit,
	updateIntegrationStepDraftMessage,
} from "./integrationEditor.ts";
import { Route as workspaceRoute } from "./route.tsx";
import styles from "./integrate.module.css";
import {
	buildNextStateCommitGraphRows,
	integrationHints,
	seedIntegrationState,
} from "./remoteIntegration.ts";

type Search = {
	branch: string;
};

type PreviewState =
	| { kind: "empty" }
	| { kind: "error"; message: string }
	| { kind: "success"; rows: Array<CommitGraphRow> | null };

const validateSearch = (search: Record<string, unknown>): Search => ({
	branch: typeof search.branch === "string" ? search.branch : "",
});

const displayBranchName = (branchRef: string): string => branchRef.replace(/^refs\/heads\//u, "");

const useRestoreBranchSelection = ({
	projectId,
	branchRef,
}: {
	projectId: string;
	branchRef: string;
}) => {
	const dispatch = useAppDispatch();
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));

	useEffect(() => {
		if (!headInfo || branchRef === "") return;

		const branch = findBranchByRef({
			headInfo,
			branchRef: encodeRefName(branchRef),
		});
		if (!branch) return;

		dispatch(
			projectActions.selectOutline({
				projectId,
				selection: branchOperand({
					stackId: branch.stackId,
					branchRef: branch.segment.refName?.fullNameBytes ?? encodeRefName(branchRef),
				}),
			}),
		);
	}, [branchRef, dispatch, headInfo, projectId]);
};

const TopBarActions: FC<{
	canRun: boolean;
	isPreviewPending: boolean;
	isApplyPending: boolean;
	onPreview: () => void;
	onApply: () => void;
	onCancel: () => void;
}> = ({ canRun, isPreviewPending, isApplyPending, onPreview, onApply, onCancel }) => (
	<>
		<button
			className={uiStyles.button}
			type="button"
			onClick={onPreview}
			disabled={!canRun || isPreviewPending || isApplyPending}
		>
			{isPreviewPending ? "Previewing…" : "Preview"}
		</button>
		<button
			className={uiStyles.button}
			type="button"
			onClick={onApply}
			disabled={!canRun || isApplyPending || isPreviewPending}
		>
			{isApplyPending ? "Applying…" : "Apply"}
		</button>
		<button className={uiStyles.button} type="button" onClick={onCancel} disabled={isApplyPending}>
			Cancel
		</button>
	</>
);

type StepDragData = {
	stepId: string;
};

const parseStepDragData = (data: unknown): StepDragData | null => {
	if (typeof data !== "object" || data === null || !("stepId" in data)) return null;
	return data as StepDragData;
};

const stepKinds: Array<InteractiveIntegrationStep["kind"]> = ["pick", "merge", "squash"];

const displayCommitOption = (option: CommitPickerOption): string => {
	const refs = option.refs.length === 0 ? "" : ` (${option.refs.join(", ")})`;
	return `${option.id.slice(0, 7)}${refs} ${option.subject}`;
};

const CommitOptions: FC<{
	commitOptions: Array<CommitPickerOption>;
	excludeCommitId?: string;
}> = ({ commitOptions, excludeCommitId }) => {
	const groups = ["Local", "Upstream", "Shared"] as const;
	const filteredOptions = commitOptions.filter((option) => option.id !== excludeCommitId);
	const effectiveExcludeCommitId =
		excludeCommitId !== undefined && filteredOptions.length > 0 ? excludeCommitId : undefined;

	return (
		<>
			{groups.map((group) => {
				const options = commitOptions.filter(
					(option) => option.group === group && option.id !== effectiveExcludeCommitId,
				);
				if (options.length === 0) return null;
				return (
					<optgroup key={group} label={group}>
						{options.map((option) => (
							<option key={option.id} value={option.id}>
								{displayCommitOption(option)}
							</option>
						))}
					</optgroup>
				);
			})}
		</>
	);
};

const DropSlot: FC<{
	index: number;
	onMoveStep: (draggedStepId: string, destinationIndex: number) => void;
}> = ({ index, onMoveStep }) => {
	const slotRef = useRef<HTMLDivElement>(null);
	const [isActive, setIsActive] = useState(false);
	const canDrop = useEffectEvent(
		({ source }: { source: { data: unknown } }) => parseStepDragData(source.data) !== null,
	);
	const onDrop = useEffectEvent(({ source }: { source: { data: unknown } }) => {
		const dragData = parseStepDragData(source.data);
		setIsActive(false);
		if (!dragData) return;
		onMoveStep(dragData.stepId, index);
	});

	useEffect(() => {
		const element = slotRef.current;
		if (!element) return;

		return dropTargetForElements({
			element,
			canDrop,
			getData: () => ({ index }),
			onDragEnter: () => setIsActive(true),
			onDragLeave: () => setIsActive(false),
			onDrop,
		});
	}, [index]);

	return (
		<div
			ref={slotRef}
			className={classes(styles.editorDropSlot, isActive && styles.editorDropSlotActive)}
		/>
	);
};

const StepChip: FC<{
	step: IntegrationStepDraft;
	commitOptions: Array<CommitPickerOption>;
	onChange: (nextStep: IntegrationStepDraft) => void;
	onDelete: () => void;
	onDragStateChange: (dragging: boolean) => void;
}> = ({ step, commitOptions, onChange, onDelete, onDragStateChange }) => {
	const handleRef = useRef<HTMLButtonElement>(null);
	const [isDragging, setIsDragging] = useState(false);

	useEffect(() => {
		const element = handleRef.current;
		if (!element) return;

		return draggable({
			element,
			getInitialData: (): StepDragData => ({ stepId: step.id }),
			onDragStart: () => {
				setIsDragging(true);
				onDragStateChange(true);
			},
			onDrop: () => {
				setIsDragging(false);
				onDragStateChange(false);
			},
		});
	}, [onDragStateChange, step.id]);

	return (
		<div className={classes(styles.editorChip, isDragging && styles.editorChipDragging)}>
			<div className={styles.editorChipHeader}>
				<button
					ref={handleRef}
					className={styles.editorChipHandle}
					type="button"
					aria-label="Drag step"
				>
					::
				</button>
				<button className={styles.editorChipDelete} type="button" onClick={onDelete}>
					Delete
				</button>
			</div>

			<div
				className={classes(
					styles.editorControls,
					step.kind === "squash" && styles.editorControlsSquash,
				)}
			>
				<label className={styles.editorField}>
					<select
						className={styles.editorSelect}
						value={step.kind}
						onChange={(event) =>
							onChange(
								changeIntegrationStepDraftKind({
									step,
									kind: event.target.value as InteractiveIntegrationStep["kind"],
									commitOptions,
								}),
							)
						}
					>
						{stepKinds.map((kind) => (
							<option key={kind} value={kind}>
								{kind}
							</option>
						))}
					</select>
				</label>

				{step.kind === "squash" ? (
					<>
						<label className={styles.editorField}>
							<select
								className={styles.editorSelect}
								value={step.commitIds[0]}
								onChange={(event) =>
									onChange(
										updateIntegrationStepDraftCommit({
											step,
											commitId: event.target.value,
											index: 0,
											commitOptions,
										}),
									)
								}
							>
								<CommitOptions commitOptions={commitOptions} excludeCommitId={step.commitIds[1]} />
							</select>
						</label>

						<label className={styles.editorField}>
							<select
								className={styles.editorSelect}
								value={step.commitIds[1]}
								onChange={(event) =>
									onChange(
										updateIntegrationStepDraftCommit({
											step,
											commitId: event.target.value,
											index: 1,
											commitOptions,
										}),
									)
								}
							>
								<CommitOptions commitOptions={commitOptions} excludeCommitId={step.commitIds[0]} />
							</select>
						</label>

						<label className={classes(styles.editorField, styles.editorFieldWide)}>
							<span className={styles.editorLabel}>Message</span>
							<input
								className={styles.editorInput}
								type="text"
								value={step.message}
								placeholder="Optional squash message"
								onChange={(event) =>
									onChange(
										updateIntegrationStepDraftMessage({
											step,
											message: event.target.value,
										}),
									)
								}
							/>
						</label>
					</>
				) : (
					<label className={styles.editorField}>
						<select
							className={styles.editorSelect}
							value={step.commitId}
							onChange={(event) =>
								onChange(
									updateIntegrationStepDraftCommit({
										step,
										commitId: event.target.value,
										commitOptions,
									}),
								)
							}
						>
							<CommitOptions commitOptions={commitOptions} />
						</select>
					</label>
				)}
			</div>
		</div>
	);
};

const IntegrationEditor: FC<{
	steps: Array<IntegrationStepDraft>;
	commitOptions: Array<CommitPickerOption>;
	hints: string;
	onChangeSteps: (nextSteps: Array<IntegrationStepDraft>) => void;
}> = ({ steps, commitOptions, hints, onChangeSteps }) => {
	const [isDraggingAnyStep, setIsDraggingAnyStep] = useState(false);
	const canAddStep = commitOptions.length > 0;

	const updateStep = (stepId: string, nextStep: IntegrationStepDraft) =>
		onChangeSteps(steps.map((step) => (step.id === stepId ? nextStep : step)));

	const deleteStep = (stepId: string) => onChangeSteps(steps.filter((step) => step.id !== stepId));

	const addStep = () => {
		if (!canAddStep) return;
		onChangeSteps([...steps, createDefaultIntegrationStepDraft(commitOptions)]);
	};

	const moveStep = (draggedStepId: string, destinationIndex: number) =>
		onChangeSteps(
			reorderIntegrationStepDrafts({
				steps,
				draggedStepId,
				destinationIndex,
			}),
		);

	return (
		<div className={styles.editorLayout}>
			<div className={styles.editorList}>
				<DropSlot index={0} onMoveStep={moveStep} />
				{steps.length === 0 ? (
					<div className={styles.editorEmpty}>
						{canAddStep
							? "No integration steps yet."
							: "No current-state commits are available to build integration steps."}
					</div>
				) : null}

				{steps.map((step, index) => (
					<div key={step.id}>
						<StepChip
							step={step}
							commitOptions={commitOptions}
							onChange={(nextStep) => updateStep(step.id, nextStep)}
							onDelete={() => deleteStep(step.id)}
							onDragStateChange={setIsDraggingAnyStep}
						/>
						<DropSlot index={index + 1} onMoveStep={moveStep} />
					</div>
				))}
			</div>

			<div className={styles.editorFooter}>
				<div className={styles.editorAddRow}>
					<button
						className={uiStyles.button}
						type="button"
						onClick={addStep}
						disabled={!canAddStep}
					>
						Add step
					</button>
					<span className={styles.editorLabel}>
						{isDraggingAnyStep ? "Drop between chips to reorder" : "Drag the handle to reorder"}
					</span>
				</div>
				<pre className={styles.editorHints}>{hints}</pre>
			</div>
		</div>
	);
};

const LoadedIntegratePage: FC<{
	projectId: string;
	branch: string;
	initialIntegration: InitialBranchIntegration;
}> = ({ projectId, branch, initialIntegration }) => {
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();
	const seeded = seedIntegrationState(initialIntegration);
	const commitOptions = buildCommitPickerOptions(initialIntegration);
	const [steps, setSteps] = useState<Array<IntegrationStepDraft>>(
		buildIntegrationStepDrafts(initialIntegration.integration),
	);
	const [dirty, setDirty] = useState(false);
	const [previewState, setPreviewState] = useState<PreviewState>({ kind: "empty" });

	useRestoreBranchSelection({ projectId, branchRef: branch });

	const currentStateRows = seeded.currentStateRows;

	const goBack = () => {
		void navigate({
			to: "/project/$id/workspace",
			params: { id: projectId },
		});
	};

	const previewMutation = useMutation({
		mutationFn: window.lite.applyBranchIntegration,
		onSuccess: (result) => {
			setPreviewState({
				kind: "success",
				rows: buildNextStateCommitGraphRows({ workspace: result.workspace, branchRef: branch }),
			});
		},
		onError: (error) => {
			setPreviewState({
				kind: "error",
				message: errorMessageForToast(error),
			});
		},
	});

	const applyMutation = useMutation({
		mutationFn: window.lite.applyBranchIntegration,
		onSuccess: async (result) => {
			queryClient.setQueryData(headInfoQueryOptions(projectId).queryKey, result.workspace.headInfo);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId,
					replacedCommits: result.workspace.replacedCommits,
					headInfo: result.workspace.headInfo,
				}),
			);
			await queryClient.invalidateQueries();
			goBack();
		},
		onError: (error) => {
			const message = errorMessageForToast(error);
			setPreviewState({ kind: "error", message });
			toastManager.add({
				type: "error",
				title: "Failed to integrate branch",
				description: message,
				priority: "high",
			});
		},
	});

	const buildIntegration = () => {
		try {
			return buildInteractiveIntegration({
				mergeBase: initialIntegration.integration.mergeBase,
				steps,
			});
		} catch (error) {
			setPreviewState({
				kind: "error",
				message: errorMessageForToast(error),
			});
			return null;
		}
	};

	const runPreview = () => {
		const integration = buildIntegration();
		if (!integration) return;

		previewMutation.mutate({
			projectId,
			branchRef: branch,
			integration,
			dryRun: true,
		});
	};

	const runApply = () => {
		const integration = buildIntegration();
		if (!integration) return;

		applyMutation.mutate({
			projectId,
			branchRef: branch,
			integration,
			dryRun: false,
		});
	};

	const onStepsChange = (nextSteps: Array<IntegrationStepDraft>) => {
		setSteps(nextSteps);
		setDirty(true);
		setPreviewState({ kind: "empty" });
	};

	return (
		<div className={styles.page}>
			<section className={styles.panel}>
				<header className={styles.panelHeader}>
					<h2>Current State</h2>
					<p>{displayBranchName(branch)}</p>
				</header>
				<div className={styles.graphPanel}>
					<CommitGraph rows={currentStateRows} />
				</div>
			</section>

			<section className={styles.panel}>
				<header className={styles.panelHeader}>
					<div className={styles.panelHeaderTop}>
						<div>
							<h2>Editor</h2>
							<p>{dirty ? "Unsaved preview edits" : "Using generated integration plan"}</p>
						</div>
						<div className={styles.panelHeaderActions}>
							<TopBarActions
								canRun={steps.length > 0}
								isPreviewPending={previewMutation.isPending}
								isApplyPending={applyMutation.isPending}
								onPreview={runPreview}
								onApply={runApply}
								onCancel={goBack}
							/>
						</div>
					</div>
				</header>
				<IntegrationEditor
					steps={steps}
					commitOptions={commitOptions}
					hints={integrationHints}
					onChangeSteps={onStepsChange}
				/>
			</section>

			<section className={styles.panel}>
				<header className={styles.panelHeader}>
					<h2>Next State</h2>
					<p>Dry-run preview of the local branch outcome</p>
				</header>
				<div className={styles.graphPanel}>
					{previewState.kind === "empty" ? (
						<pre className={styles.graphFallback}>
							No preview yet.{"\n\n"}Edit the script and run Preview.
						</pre>
					) : previewState.kind === "error" ? (
						<pre className={styles.graphFallback}>{previewState.message}</pre>
					) : previewState.rows === null ? (
						<pre className={styles.graphFallback}>
							Preview succeeded, but the selected branch could not be located in the preview
							workspace.
						</pre>
					) : previewState.rows.length === 0 ? (
						<pre className={styles.graphFallback}>No commits reachable from the local branch.</pre>
					) : (
						<CommitGraph rows={previewState.rows} />
					)}
				</div>
			</section>
		</div>
	);
};

const IntegratePage: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace/integrate" });
	const { branch } = useSearch({ from: "/project/$id/workspace/integrate" });
	const navigate = useNavigate();
	const goBack = () => {
		void navigate({
			to: "/project/$id/workspace",
			params: { id: projectId },
		});
	};

	const initialIntegrationQuery = useQuery({
		...initialBranchIntegrationQueryOptions({ projectId, branchRef: branch }),
		enabled: branch !== "",
	});

	if (branch === "")
		return (
			<div className={styles.errorState}>
				<h2>Remote integration unavailable</h2>
				<p>No branch was selected for integration.</p>
				<button className={uiStyles.button} type="button" onClick={goBack}>
					Back to workspace
				</button>
			</div>
		);

	if (initialIntegrationQuery.isPending)
		return <div className={styles.loadingState}>Loading remote integration…</div>;

	if (initialIntegrationQuery.isError)
		return (
			<div className={styles.errorState}>
				<h2>Remote integration unavailable</h2>
				<p>{errorMessageForToast(initialIntegrationQuery.error)}</p>
				<button className={uiStyles.button} type="button" onClick={goBack}>
					Back to workspace
				</button>
			</div>
		);

	return (
		<LoadedIntegratePage
			key={`${projectId}:${branch}:${initialIntegrationQuery.data.integration.mergeBase}`}
			projectId={projectId}
			branch={branch}
			initialIntegration={initialIntegrationQuery.data}
		/>
	);
};

export const Route = createRoute({
	getParentRoute: () => workspaceRoute,
	path: "integrate",
	validateSearch,
	component: IntegratePage,
});
