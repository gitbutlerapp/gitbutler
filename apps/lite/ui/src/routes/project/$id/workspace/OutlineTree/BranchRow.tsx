import rowStyles from "../Row.module.css";
import {
	useBranchCreate,
	useCommitInsertBlank,
	useRemoveBranch,
	useTearOffBranch,
	useUpdateBranchName,
	useWorkspaceBranchAndAncestorsPush,
} from "#ui/api/mutations.ts";
import { forgeInfoOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { Button, Toast, Tooltip } from "@base-ui/react";
import { Toolbar } from "@base-ui/react/toolbar";
import { BranchReference, InsertSide, PushStatus, RelativeTo } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import { ComponentProps, FC, useOptimistic, useTransition } from "react";
import { classes } from "#ui/components/classes.ts";
import { GraphSegment, type GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { errorMessageForToast } from "#ui/errors.ts";
import { outlineHotkeys, selectionOperationHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { keyboardTransferOperationMode } from "#ui/outline/mode.ts";
import { branchOperand, operandEquals, type BranchOperand } from "#ui/operands.ts";
import { projectActions, selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { prForgeUrl } from "#ui/pr.ts";
import { RowLabel, RowLabelContainer, RowToolbar } from "../Row.tsx";
import { getRowButtonClassName } from "../Row-utils.ts";
import { InlineEditor } from "./InlineEditor.tsx";
import { commitMessageInputId } from "./CommitForm.tsx";
import { insertBlankCommitMenuItem } from "./insertBlankCommitMenuItem.ts";
import { ItemRow } from "./ItemRow.tsx";
import { type PartialStackState, partialStackPushDisabled } from "./partialStackState.ts";
import styles from "./BranchRow.module.css";

const focusCommitMessageInput = () => {
	document.getElementById(commitMessageInputId)?.focus();
};

export const BranchRow: FC<
	{
		projectId: string;
		refName: BranchReference;
		stackId: string;
		isCommitTarget: boolean;
		canTearOffBranch: boolean;
		canRemoveBranch: boolean;
		partialStackState: PartialStackState;
		pushStatus: PushStatus;
		graphStatus: GraphSegmentStatus;
		pullRequest: number | null;
		bottomRelativeTo: RelativeTo | null;
		isTopSegment: boolean;
	} & ComponentProps<"div">
> = ({
	projectId,
	refName,
	stackId,
	isCommitTarget,
	canTearOffBranch,
	canRemoveBranch,
	partialStackState,
	pushStatus,
	graphStatus,
	pullRequest,
	bottomRelativeTo,
	isTopSegment,
	...restProps
}) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const mforgeUrl = pullRequest !== null ? forgeInfo && prForgeUrl(pullRequest, forgeInfo) : null;

	const dispatch = useAppDispatch();
	const branchOperandV: BranchOperand = {
		stackId,
		branchRef: refName.fullNameBytes,
	};
	const operand = branchOperand(branchOperandV);
	const isDefaultMode = useAppSelector(
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);
	const isRenaming = useAppSelector((state) => {
		const outlineMode = selectProjectOutlineModeState(state, projectId);
		return (
			outlineMode._tag === "RenameBranch" &&
			operandEquals(operand, branchOperand(outlineMode.operand))
		);
	});
	const [optimisticBranchDisplayName, setOptimisticBranchDisplayName] = useOptimistic(
		refName.displayName,
		(_currentBranchName, nextBranchName: string) => nextBranchName,
	);
	const [isRenamePending, startRenameTransition] = useTransition();

	const updateBranchNameMutation = useUpdateBranchName({
		projectId,
		stackId,
		branchRef: refName.fullNameBytes,
		oldBranch: branchOperandV,
	});

	const startEditing = () => {
		dispatch(projectActions.startRenameBranch({ projectId, branch: branchOperandV }));
	};

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		focusSelectionScope("outline");
	};

	const toastManager = Toast.useToastManager();

	const workspaceBranchAndAncestorsPushMutation = useWorkspaceBranchAndAncestorsPush();
	const commitInsertBlankMutation = useCommitInsertBlank();
	const tearOffBranchMutation = useTearOffBranch();
	const removeBranchMutation = useRemoveBranch();
	const branchCreateMutation = useBranchCreate();

	const pushesMultipleBranches = partialStackState.branchCount > 1;

	const saveBranchName = (newBranchName: string) => {
		const trimmed = newBranchName.trim();
		if (trimmed === "" || trimmed === refName.displayName) return;
		startRenameTransition(async () => {
			setOptimisticBranchDisplayName(trimmed);
			try {
				await updateBranchNameMutation.mutateAsync({
					projectId,
					stackId,
					branchName: refName.displayName,
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

	const relativeTo: RelativeTo = { type: "referenceBytes", subject: refName.fullNameBytes };
	const bucketRelativeTo = (side: InsertSide): RelativeTo =>
		side === "below" && bottomRelativeTo !== null ? bottomRelativeTo : relativeTo;

	const setCommitTarget = () => {
		dispatch(projectActions.setCommitTarget({ projectId, commitTarget: relativeTo }));
	};

	const composeCommitHere = () => {
		setCommitTarget();
		focusCommitMessageInput();
	};

	const cutBranch = () => {
		dispatch(
			projectActions.enterTransferMode({
				projectId,
				mode: keyboardTransferOperationMode({
					source: operand,
					operationType: "into",
				}),
			}),
		);
		focusSelectionScope("outline");
	};

	const insertBlankCommit = (side: "above" | "below") => {
		commitInsertBlankMutation.mutate({
			projectId,
			relativeTo,
			side,
			dryRun: false,
		});
	};

	const createDependentBranch = (side: "above" | "below") => {
		branchCreateMutation.mutate(
			{
				projectId,
				newRef: null,
				placement: {
					type: "dependent",
					subject: {
						relativeTo: bucketRelativeTo(side),
						side,
					},
				},
			},
			{
				onSuccess: (response) => {
					const newBranchStack = getHeadInfoIndex(
						response.workspace.headInfo,
					).branchContextByRefBytes(response.newRef.fullNameBytes)?.stack;

					if (newBranchStack && newBranchStack.id !== null)
						dispatch(
							projectActions.selectOutline({
								projectId,
								selection: branchOperand({
									stackId: newBranchStack.id,
									branchRef: response.newRef.fullNameBytes,
								}),
							}),
						);
				},
			},
		);
	};

	const tearOffBranch = () => {
		tearOffBranchMutation.mutate({
			projectId,
			subjectBranch: decodeBytes(refName.fullNameBytes),
			dryRun: false,
		});
	};

	const workspaceBranchAndAncestorsPush = () => {
		workspaceBranchAndAncestorsPushMutation.mutate({
			projectId,
			branch: decodeBytes(refName.fullNameBytes),
			withForce: partialStackState.pushWithForce,
			skipForcePushProtection: false,
			runHooks: true,
			pushOpts: [],
		});
	};

	const openPRInBrowser = async (): Promise<void> => {
		if (mforgeUrl == null) return;

		await window.lite.openInWebBrowser(mforgeUrl);
	};

	const workspaceBranchAndAncestorsPushDisabled =
		workspaceBranchAndAncestorsPushMutation.isPending ||
		partialStackPushDisabled(partialStackState);

	const pushMenuLabel = pushesMultipleBranches
		? partialStackState.pushWithForce
			? "Force Push With Branches Below"
			: "Push With Branches Below"
		: partialStackState.pushWithForce
			? "Force Push Branch"
			: "Push Branch";

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: pushMenuLabel,
			enabled: !workspaceBranchAndAncestorsPushDisabled,
			accelerator: toElectronAccelerator(outlineHotkeys.workspaceBranchAndAncestorsPush.hotkey),
			onSelect: workspaceBranchAndAncestorsPush,
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Rename Branch",
			enabled: !isRenamePending,
			accelerator: toElectronAccelerator(outlineHotkeys.renameBranch.hotkey),
			onSelect: startEditing,
		}),
		nativeMenuItem({
			label: "Cut Branch",
			onSelect: cutBranch,
			accelerator: toElectronAccelerator(selectionOperationHotkeys.cut.hotkey),
		}),
		nativeMenuItem({
			label: "Copy Branch Name",
			onSelect: () => window.lite.clipboardWriteText(optimisticBranchDisplayName),
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Compose Commit Here",
			accelerator: toElectronAccelerator(outlineHotkeys.composeCommitHere.hotkey),
			onSelect: composeCommitHere,
			enabled: isDefaultMode,
		}),
		nativeMenuItem({
			label: "Set Commit Target",
			accelerator: toElectronAccelerator(outlineHotkeys.setCommitTarget.hotkey),
			onSelect: setCommitTarget,
			enabled: isDefaultMode,
		}),
		nativeMenuItem({
			label: "Open In Browser",
			enabled: mforgeUrl != null,
			accelerator: toElectronAccelerator(outlineHotkeys.openPRInBrowser.hotkey),
			onSelect: openPRInBrowser,
		}),
		insertBlankCommitMenuItem(insertBlankCommit, "below"),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Create Branch",
			submenu: [
				nativeMenuItem({
					label: "Above",
					accelerator: toElectronAccelerator(outlineHotkeys.createDependentBranchAbove.hotkey),
					onSelect: () => createDependentBranch("above"),
				}),
				nativeMenuItem({
					label: "Below",
					onSelect: () => createDependentBranch("below"),
				}),
			],
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Tear Off Branch",
			enabled: canTearOffBranch && !tearOffBranchMutation.isPending,
			onSelect: tearOffBranch,
		}),
		nativeMenuItem({
			label: "Delete Branch Reference",
			enabled: canRemoveBranch,
			onSelect: () =>
				removeBranchMutation.mutate({
					projectId,
					stackId,
					branchName: decodeBytes(refName.fullNameBytes),
				}),
		}),
	];

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			operand={operand}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
			isCommitTarget={isCommitTarget}
		>
			<GraphSegment glyph={isTopSegment ? "forkRight" : "joinRight"} status={graphStatus} />

			{isRenaming ? (
				<InlineEditor
					multiline={false}
					heading
					value={optimisticBranchDisplayName}
					label="Branch name"
					onMount={(el) => {
						el.select();
					}}
					onSubmit={saveBranchName}
					onExit={endEditing}
				/>
			) : (
				<div className={styles.label}>
					<RowLabelContainer>
						<RowLabel heading>{optimisticBranchDisplayName}</RowLabel>
					</RowLabelContainer>

					<div className={classes("text-13", styles.labelMeta)}>
						<span className={classes(rowStyles.fadedText, styles.labelMetaItem)}>
							{Match.value(pushStatus).pipe(
								Match.when("nothingToPush", () => "Nothing to push"),
								Match.when("unpushedCommits", () => "Some unpushed"),
								Match.when("completelyUnpushed", () => "Unpushed branch"),
								Match.when("unpushedCommitsRequiringForce", () => "Some unpushed"),
								Match.when("integrated", () => "Integrated"),
								Match.exhaustive,
							)}
						</span>

						{pullRequest !== null && (
							<span className={classes(rowStyles.fadedText, styles.labelMetaItem)}>
								<Icon name="pr" />
								PR
							</span>
						)}

						{partialStackState.requiresPush &&
							(() => {
								const workspaceBranchAndAncestorsPushDisabledReason =
									workspaceBranchAndAncestorsPushMutation.isPending
										? "pushing"
										: partialStackState.hasConflicts
											? "disabled due to conflicts"
											: null;

								const pushButtonLabel = `${
									pushesMultipleBranches
										? partialStackState.pushWithForce
											? "Force push this and all branches below"
											: "Push this and all branches below"
										: partialStackState.pushWithForce
											? "Force push branch"
											: "Push branch"
								}${workspaceBranchAndAncestorsPushDisabledReason !== null ? ` (${workspaceBranchAndAncestorsPushDisabledReason})` : ""}`;

								return (
									<Tooltip.Root>
										<Tooltip.Trigger
											aria-label={pushButtonLabel}
											onClick={workspaceBranchAndAncestorsPush}
											className={getRowButtonClassName({ variant: "outline" })}
											// We pass `disabled` here because we want to disable the button, not
											// the tooltip. Other props should be passed above.
											render={
												<Button
													focusableWhenDisabled
													disabled={workspaceBranchAndAncestorsPushDisabled}
												/>
											}
										>
											Push
											{workspaceBranchAndAncestorsPushMutation.isPending ? (
												<Icon name="spinner" />
											) : pushesMultipleBranches ? (
												<Icon size={12} name="arrow-double-up" />
											) : (
												<Icon size={12} name="arrow-up" />
											)}
										</Tooltip.Trigger>
										<Tooltip.Portal>
											<Tooltip.Positioner sideOffset={4}>
												<Tooltip.Popup
													render={
														<TooltipPopup
															kbd={outlineHotkeys.workspaceBranchAndAncestorsPush.hotkey}
														/>
													}
												>
													{pushButtonLabel}
												</Tooltip.Popup>
											</Tooltip.Positioner>
										</Tooltip.Portal>
									</Tooltip.Root>
								);
							})()}
					</div>
				</div>
			)}

			{isDefaultMode && (
				<Toolbar.Root aria-label="Branch actions" render={<RowToolbar />}>
					<Toolbar.Button
						aria-label="Branch menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			)}
		</ItemRow>
	);
};
