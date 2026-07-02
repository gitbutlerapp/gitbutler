import rowStyles from "../Row.module.css";
import {
	useBranchCreate,
	useCommitAmend,
	useCommitDiscard,
	useCommitInsertBlank,
	useCommitReword,
	useCommitUncommit,
} from "#ui/api/mutations.ts";
import { forgeInfoOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { Checkbox } from "#ui/components/Checkbox.tsx";
import { classes } from "#ui/components/classes.ts";
import { GraphSegment } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { assert } from "#ui/assert.ts";
import { commitBody, commitForgeUrl, commitIsDiverged, commitTitle } from "#ui/commit.ts";
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
import { branchOperand, commitOperand, operandEquals, type CommitOperand } from "#ui/operands.ts";
import {
	projectActions,
	selectProjectCommitChecked,
	selectProjectHighlightedCommitIds,
	selectProjectOutlineModeState,
} from "#ui/projects/state.ts";
import { rewrittenCommitSelection } from "#ui/projects/workspace/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { RelativeTo, type Commit } from "@gitbutler/but-sdk";
import { Toast, Tooltip } from "@base-ui/react";
import { Toolbar } from "@base-ui/react/toolbar";
import { useQuery } from "@tanstack/react-query";
import { ComponentProps, FC, use, useOptimistic, useTransition } from "react";
import { RowLabel, RowLabelContainer, RowToolbar } from "../Row.tsx";
import { getRowButtonClassName } from "../Row-utils.ts";
import { NavigationIndexContext } from "../OutlineNavigationIndexContext.ts";
import { commitMessageInputId } from "./CommitForm.tsx";
import { InlineEditor } from "./InlineEditor.tsx";
import { insertBlankCommitMenuItem } from "./insertBlankCommitMenuItem.ts";
import { ItemRow } from "./ItemRow.tsx";
import { selectAfterDiscardedCommit } from "./selectAfterDiscardedCommit.ts";
import styles from "./CommitRow.module.css";

const focusCommitMessageInput = () => {
	document.getElementById(commitMessageInputId)?.focus();
};

export const CommitRow: FC<
	{
		commit: Commit;
		projectId: string;
		stackId: string;
		isCommitTarget: boolean;
		dryRunCommit: Commit | null;
	} & ComponentProps<"div">
> = ({ commit, projectId, stackId, isCommitTarget, dryRunCommit, ...restProps }) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const mforgeUrl = forgeInfo && commitForgeUrl(commit, forgeInfo);

	const isHighlighted = useAppSelector((state) =>
		selectProjectHighlightedCommitIds(state, projectId).includes(commit.id),
	);
	const isChecked = useAppSelector((state) =>
		selectProjectCommitChecked(state, projectId, commit.id),
	);

	const dispatch = useAppDispatch();
	const navigationIndex = assert(use(NavigationIndexContext));
	const commitOperandV: CommitOperand = {
		stackId,
		commitId: commit.id,
	};
	const operand = commitOperand(commitOperandV);
	const isDefaultMode = useAppSelector(
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);
	const isRewording = useAppSelector((state) => {
		const outlineMode = selectProjectOutlineModeState(state, projectId);
		return (
			outlineMode._tag === "RewordCommit" &&
			operandEquals(operand, commitOperand(outlineMode.operand))
		);
	});
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

	const commitInsertBlankMutation = useCommitInsertBlank();
	const commitDiscardMutation = useCommitDiscard();
	const commitUncommitMutation = useCommitUncommit();
	const commitRewordMutation = useCommitReword();
	const commitAmendMutation = useCommitAmend({ projectId });
	const branchCreateMutation = useBranchCreate();

	const insertBlankCommit = (side: "above" | "below") => {
		commitInsertBlankMutation.mutate({
			projectId,
			relativeTo: { type: "commit", subject: commit.id },
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
						relativeTo: { type: "commit", subject: commit.id },
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

	const deleteCommit = () => {
		const selectionAfterDiscard = selectAfterDiscardedCommit({
			navigationIndex,
			commit: commitOperandV,
		});

		commitDiscardMutation.mutate(
			{
				projectId,
				subjectCommitId: commit.id,
				dryRun: false,
			},
			{
				onSuccess: (response) => {
					dispatch(
						projectActions.selectOutline({
							projectId,
							selection: rewrittenCommitSelection({
								selection: selectionAfterDiscard,
								replacedCommits: response.workspace.replacedCommits,
								headInfo: response.workspace.headInfo,
							}),
						}),
					);
				},
			},
		);
	};

	const cutCommit = () => {
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

	const startEditing = () => {
		dispatch(projectActions.startRewordCommit({ projectId, commit: commitOperandV }));
	};

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		focusSelectionScope("outline");
	};

	const toastManager = Toast.useToastManager();

	const saveNewMessage = (newMessage: string) => {
		const initialMessage = commit.message.trim();
		const trimmed = newMessage.trim();
		if (trimmed === initialMessage) return;
		startCommitMessageTransition(async () => {
			setOptimisticMessage(trimmed);
			try {
				await commitRewordMutation.mutateAsync({
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

	const relativeTo: RelativeTo = { type: "commit", subject: commit.id };

	const amendCommit = () => {
		commitAmendMutation.mutate({ commitId: commit.id });
	};

	const setCommitTarget = () => {
		dispatch(projectActions.setCommitTarget({ projectId, commitTarget: relativeTo }));
	};

	const composeCommitHere = () => {
		setCommitTarget();
		focusCommitMessageInput();
	};

	const openCommitInBrowser = async (): Promise<void> => {
		if (!mforgeUrl) return;

		await window.lite.openInWebBrowser(mforgeUrl.url);
	};

	const title = commitTitle(commitWithOptimisticMessage.message);
	const body = commitBody(commitWithOptimisticMessage.message);

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: "Reword Commit",
			enabled: !isCommitMessagePending,
			accelerator: toElectronAccelerator(outlineHotkeys.rewordCommit.hotkey),
			onSelect: startEditing,
		}),
		nativeMenuItem({
			label: "Amend Commit",
			accelerator: toElectronAccelerator(outlineHotkeys.amendCommit.hotkey),
			enabled: isDefaultMode && !commitAmendMutation.isPending,
			onSelect: amendCommit,
		}),
		nativeMenuItem({
			label: "Cut Commit",
			onSelect: cutCommit,
			accelerator: toElectronAccelerator(selectionOperationHotkeys.cut.hotkey),
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
			label: "Copy",
			submenu: [
				nativeMenuItem({
					label: "Change ID",
					onSelect: () => window.lite.clipboardWriteText(commit.changeId),
				}),
				nativeMenuItem({
					label: "Commit ID",
					onSelect: () => window.lite.clipboardWriteText(commit.id),
				}),
				nativeMenuItem({
					label: "Commit Title",
					enabled: title !== undefined,
					onSelect: () => window.lite.clipboardWriteText(title ?? ""),
				}),
				nativeMenuItem({
					label: "Commit Body",
					enabled: body !== undefined,
					onSelect: () => window.lite.clipboardWriteText(body ?? ""),
				}),
			],
		}),
		nativeMenuItem({
			label: mforgeUrl?.freshness === "stale" ? "Open In Browser (stale)" : "Open In Browser",
			enabled: mforgeUrl != null,
			accelerator: toElectronAccelerator(outlineHotkeys.openCommitInBrowser.hotkey),
			onSelect: openCommitInBrowser,
		}),
		insertBlankCommitMenuItem(insertBlankCommit, "above"),
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
			label: "Delete Commit",
			enabled: !commitDiscardMutation.isPending,
			accelerator: toElectronAccelerator(outlineHotkeys.deleteCommit.hotkey),
			onSelect: deleteCommit,
		}),
		nativeMenuItem({
			label: "Uncommit",
			enabled: !commitUncommitMutation.isPending,
			onSelect: () =>
				commitUncommitMutation.mutate({
					projectId,
					assignTo: null,
					subjectCommitIds: [commit.id],
					dryRun: false,
				}),
		}),
	];

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			operand={operand}
			isHighlighted={isHighlighted}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
			className={classes(restProps.className, styles.row)}
			isCommitTarget={isCommitTarget}
		>
			<div className={styles.graphSegmentWithCheckbox}>
				<GraphSegment
					glyph="commit"
					status={commitIsDiverged(commit) ? "Diverged" : commit.state.type}
				/>
				<Tooltip.Root
					// This gets in the way when the user tries to move their hover to a
					// sibling row.
					disableHoverablePopup
				>
					<Checkbox
						disabled={!isDefaultMode}
						aria-label={`Check commit ${title ?? "(no message)"}`}
						checked={isChecked}
						className={styles.checkbox}
						nativeButton
						render={<Tooltip.Trigger />}
						onCheckedChange={(checked) => {
							dispatch(
								projectActions.setCommitChecked({ projectId, commitId: commit.id, checked }),
							);
						}}
					/>
					<Tooltip.Portal>
						<Tooltip.Positioner sideOffset={4}>
							<Tooltip.Popup render={<TooltipPopup kbd={outlineHotkeys.checkCommit.hotkey} />}>
								{outlineHotkeys.checkCommit.meta.name}
							</Tooltip.Popup>
						</Tooltip.Positioner>
					</Tooltip.Portal>
				</Tooltip.Root>
			</div>

			{isRewording ? (
				<InlineEditor
					multiline
					value={optimisticMessage.trim()}
					label="Commit message"
					onMount={(el) => {
						const firstNewline = el.value.indexOf("\n");
						const cursorPosition = firstNewline !== -1 ? firstNewline : el.value.length;
						el.setSelectionRange(cursorPosition, cursorPosition);
					}}
					onSubmit={saveNewMessage}
					onExit={endEditing}
				/>
			) : (
				<RowLabelContainer>
					<RowLabel singleLine>
						{title === undefined ? (
							<span className={rowStyles.fadedText}>(no message)</span>
						) : (
							title
						)}
						{hasConflicts && " ⚠️"}
					</RowLabel>
				</RowLabelContainer>
			)}

			{isDefaultMode && (
				<Toolbar.Root aria-label="Commit actions" render={<RowToolbar />}>
					<Toolbar.Button
						aria-label="Commit menu"
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
