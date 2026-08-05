import rowStyles from "../Row.module.css";
import {
	useBranchCreate,
	useCommitDiscard,
	useCommitInsertBlank,
	useCommitReword,
	useCommitUncommit,
} from "#ui/api/mutations.ts";
import { forgeInfoOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/components/classes.ts";
import { ConflictIcon } from "#ui/components/ConflictIcon.tsx";
import { GraphSegment } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { assert } from "#ui/assert.ts";
import { commitBody, commitForgeUrl, commitIsDiverged, commitTitle } from "#ui/commit.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import {
	changesHotkeys,
	outlineHotkeys,
	selectionOperationHotkeys,
	toElectronAccelerator,
} from "#ui/hotkeys.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { branchOperand, commitOperand, operandEquals, type CommitOperand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import type { Commit } from "@gitbutler/but-sdk";
import { Toast, Toolbar, Tooltip } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { type ComponentProps, type FC, use, useOptimistic, useTransition } from "react";
import { RowCheckbox, RowLabel, RowLabelContainer, RowToolbar } from "../Row.tsx";
import { getRowButtonClassName } from "../Row-utils.ts";
import { NavigationIndexContext } from "../OutlineNavigationIndexContext.ts";
import { InlineEditor } from "./InlineEditor.tsx";
import { insertBlankCommitMenuItem } from "./insertBlankCommitMenuItem.ts";
import { ItemRow } from "./ItemRow.tsx";
import { selectAfterDiscardedCommit } from "./selectAfterDiscardedCommit.ts";
import styles from "./CommitRow.module.css";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";

export const CommitRow: FC<
	{
		commit: Commit;
		projectId: string;
		dryRunCommit: Commit | null;
		checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
		amendCommit: () => void;
		canAmendCommit: boolean;
	} & ComponentProps<"div">
> = ({
	commit,
	projectId,
	dryRunCommit,
	checkCommit,
	amendCommit,
	canAmendCommit,
	...restProps
}) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const mforgeUrl = forgeInfo && commitForgeUrl(commit, forgeInfo);
	const commitOperandV: CommitOperand = {
		commitId: commit.id,
		changeId: commit.changeId,
	};
	const operand = commitOperand(commitOperandV);

	const canCheck = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckCommits(state, projectId),
	);
	const isHighlighted = useAppSelector((state) =>
		projectSlice.selectors.selectHighlightedCommitIds(state, projectId).includes(commit.id),
	);
	const isChecked = useAppSelector((state) =>
		projectSlice.selectors.selectOperandChecked(state, projectId, operand),
	);

	const dispatch = useAppDispatch();
	const navigationIndex = assert(use(NavigationIndexContext));
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);
	const isRewording = useAppSelector((state) => {
		const outlineMode = projectSlice.selectors.selectOutlineModeState(state, projectId);
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

	const { mutate: commitInsertBlank } = useCommitInsertBlank();
	const { isPending: isCommitDiscardPending, mutate: commitDiscard } = useCommitDiscard();
	const { isPending: isCommitUncommitPending, mutate: commitUncommit } = useCommitUncommit();
	const { mutateAsync: commitReword } = useCommitReword();
	const { mutate: branchCreate } = useBranchCreate();

	const insertBlankCommit = (side: "above" | "below") => {
		commitInsertBlank({
			projectId,
			relativeTo: { type: "commit", subject: commit.id },
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
						relativeTo: { type: "commit", subject: commit.id },
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

	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});

	const deleteCommit = () => {
		const selectionAfterDiscard = selectAfterDiscardedCommit({
			navigationIndex,
			commit: commitOperandV,
			headInfoIndex,
		});

		commitDiscard(
			{
				projectId,
				subjectCommitId: commit.id,
				dryRun: false,
			},
			{
				onSuccess: (response) => {
					let latestSelectionAfterDiscard = selectionAfterDiscard;

					rewrite: if (selectionAfterDiscard?._tag === "Commit") {
						const newId = response.workspace.replacedCommits[selectionAfterDiscard.commitId];
						if (newId === undefined) break rewrite;

						latestSelectionAfterDiscard = commitOperand({
							commitId: newId,
							changeId: selectionAfterDiscard.changeId,
						});
					}

					dispatch(
						projectSlice.actions.selectOutline({
							projectId,
							selection: latestSelectionAfterDiscard,
						}),
					);
				},
			},
		);
	};

	const cutCommit = () => {
		dispatch(
			projectSlice.actions.enterKeyboardTransferMode({
				projectId,
				sources: [operand],
			}),
		);
		focusSelectionScope("outline");
	};

	const startEditing = () => {
		dispatch(projectSlice.actions.startRewordCommit({ projectId, commit: commitOperandV }));
	};

	const endEditing = () => {
		dispatch(projectSlice.actions.exitMode({ projectId }));
		dispatch(projectSlice.actions.selectOutline({ projectId, selection: operand }));
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
				await commitReword({
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
			// Advertising a hotkey defined elsewhere.
			accelerator: toElectronAccelerator(outlineHotkeys.rewordCommit.hotkey),
			onSelect: startEditing,
		}),
		nativeMenuItem({
			label: "Amend Commit",
			accelerator: toElectronAccelerator(changesHotkeys.amendCommit.hotkey),
			enabled: isDefaultMode && canAmendCommit,
			onSelect: amendCommit,
		}),
		nativeMenuItem({
			label: "Cut Commit",
			onSelect: cutCommit,
			accelerator: toElectronAccelerator(selectionOperationHotkeys.cut.hotkey),
		}),
		nativeMenuSeparator,
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
			accelerator: toElectronAccelerator(outlineHotkeys.uncommitCommit.hotkey),
			onSelect: () =>
				commitUncommit({
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
			isChecked={isChecked}
			isHighlighted={isHighlighted}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
			className={classes(restProps.className, styles.row)}
		>
			<div className={styles.iconWithCheckbox}>
				<GraphSegment
					glyph="commit"
					status={commitIsDiverged(commit) ? "Diverged" : commit.state.type}
				/>
				<Tooltip.Root
					// This gets in the way when the user tries to move their hover to a
					// sibling row.
					disableHoverablePopup
				>
					<RowCheckbox
						disabled={!isDefaultMode || !canCheck}
						aria-label={`Check commit ${title ?? "(no message)"}`}
						checked={isChecked}
						className={styles.checkbox}
						nativeButton
						render={<Tooltip.Trigger />}
						onCheckedChange={(_checked, { event }) => {
							const shiftKey =
								(event instanceof MouseEvent || event instanceof KeyboardEvent) &&
								event.shiftKey === true;
							checkCommit({ commitId: commit.id, shiftKey });
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
					{hasConflicts && (
						<ConflictIcon
							variant="conflict"
							className={styles.conflictIcon}
							aria-label="Conflicted"
						/>
					)}
					<RowLabel singleLine>
						{title === undefined ? (
							<span className={rowStyles.fadedText}>(no message)</span>
						) : (
							title
						)}
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
