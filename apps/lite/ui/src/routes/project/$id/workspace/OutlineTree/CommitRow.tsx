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
import { classes } from "#ui/components/classes.ts";
import { GraphSegment } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { CheckedCommitIdsContext } from "#ui/CheckedCommitIdsContext.ts";
import { CommitTargetContext } from "#ui/CommitTargetContext.ts";
import { HighlightedCommitIdsContext } from "#ui/HighlightedCommitIdsContext.ts";
import { assert } from "#ui/assert.ts";
import {
	commitBody,
	commitForgeUrl,
	commitIsDiverged,
	commitTitle,
	rewrittenCommitSelection,
} from "#ui/commit.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import { outlineHotkeys, selectionOperationHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { branchOperand, commitOperand, operandEquals, type CommitOperand } from "#ui/operands.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { OutlineModeContext, OutlineSelectionActionsContext } from "#ui/WorkspaceContext.ts";
import { RelativeTo, type Commit } from "@gitbutler/but-sdk";
import { Toast } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { ComponentProps, FC, use, useOptimistic, useTransition } from "react";
import { RowCheckbox, RowLabel, RowLabelContainer, RowToolbar } from "../Row.tsx";
import { getRowButtonClassName } from "../Row-utils.ts";
import { NavigationIndexContext } from "../OutlineNavigationIndexContext.ts";
import { commitMessageInputId } from "../CommitForm.tsx";
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
	const { checkedCommitIds, setCommitsChecked } = use(CheckedCommitIdsContext);
	const { setCommitTarget: updateCommitTarget } = use(CommitTargetContext);
	const { highlightedCommitIds } = use(HighlightedCommitIdsContext);
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const mforgeUrl = forgeInfo && commitForgeUrl(commit, forgeInfo);

	const isHighlighted = highlightedCommitIds.has(commit.id);
	const isChecked = checkedCommitIds.has(commit.id);

	const {
		outlineMode,
		startRewordCommit: enterRewordMode,
		enterKeyboardTransferMode,
		exitMode,
	} = use(OutlineModeContext);
	const { selectOutline } = use(OutlineSelectionActionsContext);
	const navigationIndex = assert(use(NavigationIndexContext));
	const commitOperandV: CommitOperand = {
		stackId,
		commitId: commit.id,
	};
	const operand = commitOperand(commitOperandV);
	const isDefaultMode = outlineMode._tag === "Default";
	const isRewording =
		outlineMode._tag === "RewordCommit" &&
		operandEquals(operand, commitOperand(outlineMode.operand));
	const [optimisticMessage, setOptimisticMessage] = useOptimistic(
		commit.message,
		(_currentMessage, nextMessage: string) => nextMessage,
	);
	const [isCommitMessagePending, startCommitMessageTransition] = useTransition();

	const hasConflicts = dryRunCommit?.hasConflicts ?? commit.hasConflicts;

	const { mutate: mutateCommitInsertBlank } = useCommitInsertBlank();
	const { mutate: mutateCommitDiscard, isPending: isCommitDiscardPending } = useCommitDiscard();
	const { mutate: mutateCommitUncommit, isPending: isCommitUncommitPending } = useCommitUncommit();
	const { mutateAsync: mutateCommitRewordAsync } = useCommitReword();
	const { mutate: mutateCommitAmend, isPending: isCommitAmendPending } = useCommitAmend({
		projectId,
	});
	const { mutate: mutateBranchCreate } = useBranchCreate();

	const insertBlankCommit = (side: "above" | "below") => {
		mutateCommitInsertBlank({
			projectId,
			relativeTo: { type: "commit", subject: commit.id },
			side,
			dryRun: false,
		});
	};

	const createDependentBranch = (side: "above" | "below") => {
		mutateBranchCreate(
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

	const deleteCommit = () => {
		const selectionAfterDiscard = selectAfterDiscardedCommit({
			navigationIndex,
			commit: commitOperandV,
		});

		mutateCommitDiscard(
			{
				projectId,
				subjectCommitId: commit.id,
				dryRun: false,
			},
			{
				onSuccess: (response) => {
					selectOutline(
						projectId,
						rewrittenCommitSelection({
							selection: selectionAfterDiscard,
							replacedCommits: response.workspace.replacedCommits,
							headInfo: response.workspace.headInfo,
						}),
					);
				},
			},
		);
	};

	const cutCommit = () => {
		enterKeyboardTransferMode(projectId, operand);
		focusSelectionScope("outline");
	};

	const startEditing = () => {
		enterRewordMode(projectId, commitOperandV);
	};

	const endEditing = () => {
		exitMode(projectId);
		selectOutline(projectId, operand);
	};

	const toastManager = Toast.useToastManager();

	const saveNewMessage = (newMessage: string) => {
		const initialMessage = commit.message.trim();
		const trimmed = newMessage.trim();
		if (trimmed === initialMessage) return;
		startCommitMessageTransition(async () => {
			setOptimisticMessage(trimmed);
			try {
				await mutateCommitRewordAsync({
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
		mutateCommitAmend({ commitId: commit.id });
	};

	const setCommitTarget = () => {
		updateCommitTarget(projectId, relativeTo);
	};

	const composeCommitHere = () => {
		setCommitTarget();
		focusCommitMessageInput();
	};

	const openCommitInBrowser = async (): Promise<void> => {
		if (!mforgeUrl) return;

		await window.lite.openInWebBrowser(mforgeUrl.url);
	};

	const title = commitTitle(optimisticMessage);
	const createMenuItems = (): Array<NativeMenuItem> => {
		const body = commitBody(optimisticMessage);

		return [
			nativeMenuItem({
				label: "Reword Commit",
				enabled: !isCommitMessagePending,
				accelerator: toElectronAccelerator(outlineHotkeys.rewordCommit.hotkey),
				onSelect: startEditing,
			}),
			nativeMenuItem({
				label: "Amend Commit",
				accelerator: toElectronAccelerator(outlineHotkeys.amendCommit.hotkey),
				enabled: isDefaultMode && !isCommitAmendPending,
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
				enabled: !isCommitDiscardPending,
				accelerator: toElectronAccelerator(outlineHotkeys.deleteCommit.hotkey),
				onSelect: deleteCommit,
			}),
			nativeMenuItem({
				label: "Uncommit",
				enabled: !isCommitUncommitPending,
				onSelect: () =>
					mutateCommitUncommit({
						projectId,
						assignTo: null,
						subjectCommitIds: [commit.id],
						dryRun: false,
					}),
			}),
		];
	};

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			operand={operand}
			isHighlighted={isHighlighted}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, createMenuItems());
			}}
			className={classes(restProps.className, styles.row)}
			isCommitTarget={isCommitTarget}
		>
			<div className={styles.iconWithCheckbox}>
				<GraphSegment
					glyph="commit"
					status={commitIsDiverged(commit) ? "Diverged" : commit.state.type}
				/>
				<RowCheckbox
					disabled={!isDefaultMode}
					aria-label={`Check commit ${title ?? "(no message)"}`}
					checked={isChecked}
					className={styles.checkbox}
					nativeButton
					onCheckedChange={(checked) => {
						setCommitsChecked(projectId, [commit.id], checked);
					}}
				/>
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
				<RowToolbar aria-label="Commit actions" role="toolbar">
					<button
						aria-label="Commit menu"
						type="button"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, createMenuItems());
						}}
						className={getRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</button>
				</RowToolbar>
			)}
		</ItemRow>
	);
};
