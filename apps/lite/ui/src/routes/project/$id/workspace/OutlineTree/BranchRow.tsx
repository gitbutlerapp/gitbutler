import rowStyles from "../Row.module.css";
import {
	useBranchCreate,
	useBranchRemove,
	useCommitInsertBlank,
	useTearOffBranch,
	useBranchRename,
	useWorkspaceBranchAndAncestorsPush,
} from "#ui/api/mutations.ts";
import {
	forgeInfoOptions,
	listCIChecksQueryOptions,
	listReviewsQueryOptions,
} from "#ui/api/queries.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { Button, Toast, Toolbar, Tooltip } from "@base-ui/react";
import type { BranchReference, InsertSide, PushStatus, RelativeTo } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import { type ComponentProps, type FC, type MouseEvent, useOptimistic, useTransition } from "react";
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
import { branchOperand, operandEquals, type BranchOperand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { prForgeUrl } from "#ui/pr.ts";
import { Badge, type BadgeVariant } from "#ui/components/Badge.tsx";
import {
	RowFoldToggle,
	RowLabel,
	RowLabelContainer,
	RowLabelGroup,
	RowMeta,
	RowToolbar,
} from "../Row.tsx";
import { getRowButtonClassName } from "../Row-utils.ts";
import { InlineEditor } from "./InlineEditor.tsx";
import { insertBlankCommitMenuItem } from "./insertBlankCommitMenuItem.ts";
import { ItemRow } from "./ItemRow.tsx";
import { ciChecksSummaryUrl, type AggregateCIChecks } from "#ui/ci.ts";
import { type DownstackPushStatus, downstackPushStatusDisabled } from "#ui/segment.ts";

const CIBubble: FC<{ checks: AggregateCIChecks }> = (p) => {
	switch (p.checks.status) {
		case "success":
			return (
				<Badge aria-label="CI checks succeeded" variant="safe">
					<Icon name="tick" size={12} />
				</Badge>
			);
		case "failure":
			return (
				<Badge aria-label="CI checks failed" variant="danger">
					<Icon name="cross" size={12} />
				</Badge>
			);
		case "in_progress": {
			const [variant, label]: [BadgeVariant, string] =
				p.checks.failure.length > 0
					? ["danger", "CI checks in progress, some failed"]
					: p.checks.actionRequired.length > 0
						? ["warn", "CI checks in progress, some action required"]
						: ["lightGray", "CI checks in progress"];
			return (
				<Badge aria-label={label} variant={variant}>
					<Icon name="spinner" size={12} />
				</Badge>
			);
		}
		case "cancelled":
			return (
				<Badge aria-label="CI checks cancelled" variant="lightGray">
					<Icon name="cross" size={12} />
				</Badge>
			);
		case "action_required":
			return (
				<Badge aria-label="CI checks action required" variant="warn">
					<Icon name="warning" size={12} />
				</Badge>
			);
		case "unknown":
			return (
				<Badge aria-label="CI checks status unknown" variant="lightGray">
					<Icon name="question" size={12} />
				</Badge>
			);
	}
};

export const BranchRow: FC<
	{
		projectId: string;
		refName: BranchReference;
		canTearOffBranch: boolean;
		canRemoveBranch: boolean;
		downstackPushStatus: DownstackPushStatus;
		pushStatus: PushStatus;
		graphStatus: GraphSegmentStatus;
		bottomRelativeTo: RelativeTo | null;
		isTopSegment: boolean;
		commitCount: number;
	} & ComponentProps<"div">
> = ({
	projectId,
	refName,
	canTearOffBranch,
	canRemoveBranch,
	downstackPushStatus,
	pushStatus,
	graphStatus,
	bottomRelativeTo,
	isTopSegment,
	commitCount,
	...restProps
}) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const { data: reviews } = useQuery({
		...listReviewsQueryOptions({ projectId, cacheConfig: "noCache" }),
		enabled: !!forgeInfo?.capabilities.prService,
	});
	const pullRequest = reviews?.reviewsBySourceBranch.get(refName.displayName)?.number ?? null;
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

	const dispatch = useAppDispatch();
	const branchOperandV: BranchOperand = {
		branchRef: refName.fullNameBytes,
	};
	const operand = branchOperand(branchOperandV);
	const branchRef = decodeBytes(refName.fullNameBytes);
	// A plain boolean, so this re-renders only when this branch's own fold state
	// changes rather than on every fold anywhere.
	const isFolded = useAppSelector((state) =>
		projectSlice.selectors.selectSegmentFolded(state, projectId, branchRef),
	);
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);
	const isRenaming = useAppSelector((state) => {
		const outlineMode = projectSlice.selectors.selectOutlineModeState(state, projectId);
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

	const { mutateAsync: branchRename } = useBranchRename();

	const startEditing = () => {
		dispatch(projectSlice.actions.startRenameBranch({ projectId, branch: branchOperandV }));
	};

	const endEditing = () => {
		dispatch(projectSlice.actions.exitMode({ projectId }));
		dispatch(projectSlice.actions.selectOutline({ projectId, selection: operand }));
		focusSelectionScope("outline");
	};

	const toastManager = Toast.useToastManager();

	const {
		isPending: isWorkspaceBranchAndAncestorsPushPending,
		mutate: workspaceBranchAndAncestorsPush,
	} = useWorkspaceBranchAndAncestorsPush();
	const { mutate: commitInsertBlank } = useCommitInsertBlank();
	const { isPending: isTearOffBranchPending, mutate: tearOffBranch } = useTearOffBranch();
	const { isPending: isBranchRemovePending, mutate: branchRemove } = useBranchRemove();
	const { mutate: branchCreate } = useBranchCreate();

	const pushesMultipleBranches = downstackPushStatus.downstackBranches > 1;

	const saveBranchName = (newBranchName: string) => {
		const trimmed = newBranchName.trim();
		if (trimmed === "" || trimmed === refName.displayName) return;
		startRenameTransition(async () => {
			setOptimisticBranchDisplayName(trimmed);
			try {
				await branchRename({
					projectId,
					refName: refName.fullNameBytes,
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

	const cutBranch = () => {
		dispatch(
			projectSlice.actions.enterKeyboardTransferMode({
				projectId,
				sources: [operand],
			}),
		);
		focusSelectionScope("outline");
	};

	const insertBlankCommit = (side: "above" | "below") => {
		commitInsertBlank({
			projectId,
			relativeTo,
			side,
			dryRun: false,
		});
	};

	const createDependentBranch = (side: "above" | "below") => {
		branchCreate(
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
					dispatch(
						projectSlice.actions.selectOutline({
							projectId,
							selection: branchOperand({
								branchRef: response.newRef.fullNameBytes,
							}),
						}),
					);
				},
			},
		);
	};

	const tearOff = () => {
		tearOffBranch({
			projectId,
			subjectBranch: decodeBytes(refName.fullNameBytes),
			dryRun: false,
		});
	};

	const pushBranch = () => {
		workspaceBranchAndAncestorsPush({
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
		isWorkspaceBranchAndAncestorsPushPending || downstackPushStatusDisabled(downstackPushStatus);

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
			onSelect: pushBranch,
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
			enabled: canTearOffBranch && !isTearOffBranchPending,
			onSelect: tearOff,
		}),
		nativeMenuItem({
			label: "Delete Branch Reference",
			enabled: canRemoveBranch && !isBranchRemovePending,
			accelerator: toElectronAccelerator(outlineHotkeys.deleteBranchRef.hotkey),
			onSelect: () =>
				branchRemove({
					projectId,
					refName: refName.fullNameBytes,
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
		>
			{commitCount > 0 ? (
				<RowFoldToggle
					folded={isFolded}
					aria-label={isFolded ? "Unfold commits" : "Fold commits"}
					onClick={() =>
						dispatch(projectSlice.actions.toggleSegmentFolded({ projectId, branchRef }))
					}
					glyph={
						// The glyph describes where the branch sits in the stack, so it
						// does not change with fold state.
						<GraphSegment glyph={isTopSegment ? "forkRight" : "joinRight"} status={graphStatus} />
					}
					foldedIndicator={<GraphSegment glyph="group" status={graphStatus} />}
				/>
			) : (
				<GraphSegment glyph={isTopSegment ? "forkRight" : "joinRight"} status={graphStatus} />
			)}

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
				<RowLabelGroup>
					<RowLabelContainer>
						<RowLabel heading singleLine title={optimisticBranchDisplayName}>
							{optimisticBranchDisplayName}
						</RowLabel>
					</RowLabelContainer>

					<RowMeta>
						{/* Only while folded: the count stands in for the commits it hides,
						    so showing it alongside them would just be noise. */}
						{isFolded && (
							<span className={classes(rowStyles.fadedText, rowStyles.metaItem)}>
								<Icon size={14} name="commit" />
								{commitCount}
							</span>
						)}

						<span
							className={classes(
								rowStyles.fadedText,
								rowStyles.metaItem,
								rowStyles.metaItemShrinkable,
							)}
						>
							<span className={rowStyles.metaItemText}>
								{Match.value(pushStatus).pipe(
									Match.when("nothingToPush", () => "Nothing to push"),
									Match.when("unpushedCommits", () => "Some unpushed"),
									Match.when("completelyUnpushed", () => "Unpushed branch"),
									Match.when("unpushedCommitsRequiringForce", () => "Some unpushed"),
									Match.when("integrated", () => "Integrated"),
									Match.exhaustive,
								)}
							</span>
						</span>

						{mforgeUrl != null && (
							<a
								href={mforgeUrl}
								onClick={(evt) => void openPRInBrowser(evt)}
								className={classes(rowStyles.fadedText, rowStyles.metaItem)}
							>
								<Icon size={14} name="pr" />
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
									isWorkspaceBranchAndAncestorsPushPending
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
											onClick={pushBranch}
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
											{isWorkspaceBranchAndAncestorsPushPending ? (
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
					</RowMeta>
				</RowLabelGroup>
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
