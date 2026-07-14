import rowStyles from "../Row.module.css";
import {
	useBranchCreate,
	useCommitInsertBlank,
	useRemoveBranch,
	useTearOffBranch,
	useUpdateBranchName,
	useWorkspaceBranchAndAncestorsPush,
} from "#ui/api/mutations.ts";
import { forgeInfoOptions, listCIChecksQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { Button, Toast, Tooltip } from "@base-ui/react";
import { Toolbar } from "@base-ui/react/toolbar";
import { BranchReference, InsertSide, PushStatus, RelativeTo } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import {
	type ComponentProps,
	type FC,
	type MouseEvent,
	use,
	useOptimistic,
	useTransition,
} from "react";
import { classes } from "#ui/components/classes.ts";
import { CommitTargetContext } from "#ui/CommitTargetContext.ts";
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
import { branchOperand, operandEquals, type BranchOperand } from "#ui/operands.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { OutlineModeContext, OutlineSelectionContext } from "#ui/WorkspaceContext.ts";
import { prForgeUrl } from "#ui/pr.ts";
import {
	RowBubble,
	RowBubbleVariant,
	RowLabel,
	RowLabelContainer,
	RowLabelFooter,
	RowToolbar,
} from "../Row.tsx";
import { getRowButtonClassName } from "../Row-utils.ts";
import { InlineEditor } from "./InlineEditor.tsx";
import { commitMessageInputId } from "../CommitForm.tsx";
import { insertBlankCommitMenuItem } from "./insertBlankCommitMenuItem.ts";
import { ItemRow } from "./ItemRow.tsx";
import styles from "./BranchRow.module.css";
import { ciChecksSummaryUrl, type AggregateCIChecks } from "#ui/ci.ts";
import { type DownstackPushStatus, downstackPushStatusDisabled } from "#ui/segment.ts";

const focusCommitMessageInput = () => {
	document.getElementById(commitMessageInputId)?.focus();
};

const CIBubble: FC<{ checks: AggregateCIChecks }> = (p) => {
	switch (p.checks.status) {
		case "success":
			return (
				<RowBubble aria-label="CI checks succeeded" variant="safe">
					<Icon name="tick" size={12} />
				</RowBubble>
			);
		case "failure":
			return (
				<RowBubble aria-label="CI checks failed" variant="danger">
					<Icon name="cross" size={12} />
				</RowBubble>
			);
		case "in_progress": {
			const [variant, label]: [RowBubbleVariant, string] =
				p.checks.failure.length > 0
					? ["danger", "CI checks in progress, some failed"]
					: p.checks.actionRequired.length > 0
						? ["warn", "CI checks in progress, some action required"]
						: ["lightGray", "CI checks in progress"];
			return (
				<RowBubble aria-label={label} variant={variant}>
					<Icon name="spinner" size={12} />
				</RowBubble>
			);
		}
		case "cancelled":
			return (
				<RowBubble aria-label="CI checks cancelled" variant="warn">
					<Icon name="warning" size={12} />
				</RowBubble>
			);
		case "action_required":
			return (
				<RowBubble aria-label="CI checks action required" variant="warn">
					<Icon name="warning" size={12} />
				</RowBubble>
			);
		case "unknown":
			return (
				<RowBubble aria-label="CI checks status unknown" variant="warn">
					<Icon name="warning" size={12} />
				</RowBubble>
			);
	}
};

export const BranchRow: FC<
	{
		projectId: string;
		refName: BranchReference;
		stackId: string;
		isCommitTarget: boolean;
		canTearOffBranch: boolean;
		canRemoveBranch: boolean;
		downstackPushStatus: DownstackPushStatus;
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
	downstackPushStatus,
	pushStatus,
	graphStatus,
	pullRequest,
	bottomRelativeTo,
	isTopSegment,
	...restProps
}) => {
	const { setCommitTarget: updateCommitTarget } = use(CommitTargetContext);
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const mforgeUrl = pullRequest !== null ? forgeInfo && prForgeUrl(pullRequest, forgeInfo) : null;

	const { data: ciChecks } = useQuery({
		...listCIChecksQueryOptions({
			projectId,
			reference: refName.displayName,
			polling: "passive",
		}),
		enabled: pullRequest !== null && forgeInfo?.capabilities.checks,
	});
	const ciURL =
		pullRequest !== null ? forgeInfo && ciChecksSummaryUrl(pullRequest, forgeInfo) : null;

	const {
		outlineMode,
		startRenameBranch: enterRenameMode,
		enterKeyboardTransferMode,
		exitMode,
	} = use(OutlineModeContext);
	const { selectOutline } = use(OutlineSelectionContext);
	const branchOperandV: BranchOperand = {
		stackId,
		branchRef: refName.fullNameBytes,
	};
	const operand = branchOperand(branchOperandV);
	const isDefaultMode = outlineMode._tag === "Default";
	const isRenaming =
		outlineMode._tag === "RenameBranch" &&
		operandEquals(operand, branchOperand(outlineMode.operand));
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
		enterRenameMode(projectId, branchOperandV);
	};

	const endEditing = () => {
		exitMode(projectId);
		selectOutline(projectId, operand);
	};

	const toastManager = Toast.useToastManager();

	const workspaceBranchAndAncestorsPushMutation = useWorkspaceBranchAndAncestorsPush();
	const commitInsertBlankMutation = useCommitInsertBlank();
	const tearOffBranchMutation = useTearOffBranch();
	const removeBranchMutation = useRemoveBranch();
	const branchCreateMutation = useBranchCreate();

	const pushesMultipleBranches = downstackPushStatus.downstackBranches > 1;

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
		updateCommitTarget(projectId, relativeTo);
	};

	const composeCommitHere = () => {
		setCommitTarget();
		focusCommitMessageInput();
	};

	const cutBranch = () => {
		enterKeyboardTransferMode(projectId, operand);
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

					if (newBranchStack && newBranchStack.id !== null) {
						selectOutline(
							projectId,
							branchOperand({
								stackId: newBranchStack.id,
								branchRef: response.newRef.fullNameBytes,
							}),
						);
					}
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
			withForce: downstackPushStatus.anyPushRequiresForce,
			skipForcePushProtection: false,
			runHooks: true,
			pushOpts: [],
		});
	};

	const openPRInBrowser = async (evt?: MouseEvent<HTMLAnchorElement>): Promise<void> => {
		evt?.preventDefault();

		if (mforgeUrl != null) await window.lite.openInWebBrowser(mforgeUrl);
	};

	const openCIChecksInBrowser = async (evt?: MouseEvent<HTMLAnchorElement>): Promise<void> => {
		evt?.preventDefault();

		if (ciURL != null) await window.lite.openInWebBrowser(ciURL);
	};

	const workspaceBranchAndAncestorsPushDisabled =
		workspaceBranchAndAncestorsPushMutation.isPending ||
		downstackPushStatusDisabled(downstackPushStatus);

	const pushMenuLabel = pushesMultipleBranches
		? downstackPushStatus.anyPushRequiresForce
			? "Force Push With Branches Below"
			: "Push With Branches Below"
		: downstackPushStatus.anyPushRequiresForce
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

					<RowLabelFooter className={classes("text-13", styles.labelMeta)}>
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

						{mforgeUrl != null && (
							<a
								href={mforgeUrl}
								onClick={(evt) => void openPRInBrowser(evt)}
								className={classes(rowStyles.fadedText, styles.labelMetaItem)}
							>
								<Icon name="pr" />
								PR
							</a>
						)}

						{ciChecks?.aggregate &&
							(ciURL != null ? (
								<a href={ciURL} onClick={(evt) => void openCIChecksInBrowser(evt)}>
									<CIBubble checks={ciChecks.aggregate} />
								</a>
							) : (
								<CIBubble checks={ciChecks.aggregate} />
							))}

						{downstackPushStatus.anyRequiresPush &&
							(() => {
								const workspaceBranchAndAncestorsPushDisabledReason =
									workspaceBranchAndAncestorsPushMutation.isPending
										? "pushing"
										: downstackPushStatus.anyHasConflicts
											? "disabled due to conflicts"
											: null;

								const pushButtonLabel = `${
									pushesMultipleBranches
										? downstackPushStatus.anyPushRequiresForce
											? "Force push this and all branches below"
											: "Push this and all branches below"
										: downstackPushStatus.anyPushRequiresForce
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
					</RowLabelFooter>
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
