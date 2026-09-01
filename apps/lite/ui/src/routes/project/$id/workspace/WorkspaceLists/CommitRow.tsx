import rowStyles from "../Row.module.css";
import { useAddressSpace } from "./context.tsx";
import { startKeyboardTransfer, setCursor, startInlineEdit } from "#ui/use-cursor.ts";
import {
	useBranchCreate,
	useCommitDiscard,
	useCommitInsertBlank,
	useCommitReword,
	useCommitUncommit,
	useEnterEditMode,
} from "#ui/api/mutations.ts";
import { forgeInfoOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/components/classes.ts";
import { ConflictIcon } from "#ui/components/ConflictIcon.tsx";
import { GraphSegment } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { commitBody, commitForgeUrl, commitIsDiverged, commitTitle } from "#ui/commit.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import {
	changesHotkeys,
	sidebarHotkeys,
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
import { branchAddress, commitAddress, addressEquals, type CommitAddress } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { focusScope } from "#ui/focus-scopes.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import type { Commit } from "@gitbutler/but-sdk";
import { Toast, Toolbar, Tooltip } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { type ComponentProps, type FC, useOptimistic, useTransition } from "react";
import { RowCheckbox, RowLabel, RowLabelContainer, RowToolbar } from "../Row.tsx";
import { getRowButtonClassName } from "../Row-utils.ts";
import { InlineEditor } from "./InlineEditor.tsx";
import { insertBlankCommitMenuItem } from "./insertBlankCommitMenuItem.ts";
import { ItemRow } from "./ItemRow.tsx";
import { selectAfterDiscardedCommits } from "./selectAfterDiscardedCommit.ts";
import styles from "./CommitRow.module.css";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";

export const CommitRow: FC<
	{
		commit: Commit;
		projectId: string;
		/** `null` on a stack without an id, where edit mode cannot be offered. */
		stackId: string | null;
		dryRunCommit: Commit | null;
		checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
		amendCommit: () => void;
		canAmendCommit: boolean;
		scrollSelectedIntoView?: boolean;
	} & ComponentProps<"div">
> = ({
	commit,
	projectId,
	stackId,
	dryRunCommit,
	checkCommit,
	amendCommit,
	canAmendCommit,
	...restProps
}) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const mforgeUrl = forgeInfo && commitForgeUrl(commit, forgeInfo);
	const commitAddressV: CommitAddress = {
		commitId: commit.id,
		changeId: commit.changeId,
	};
	const address = commitAddress(commitAddressV);

	const canCheck = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckCommits(state, projectId),
	);
	const isDependency = useAppSelector((state) =>
		projectSlice.selectors.selectDependencyCommitIds(state, projectId).has(commit.id),
	);
	const isChecked = useAppSelector((state) =>
		projectSlice.selectors.selectAddressChecked(state, projectId, address),
	);

	const dispatch = useAppDispatch();
	const store = useAppStore();
	const addressSpace = useAddressSpace();
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const isRewording = useAppSelector((state) => {
		const pendingOperation = projectSlice.selectors.selectPendingOperation(state, projectId);
		return (
			pendingOperation._tag === "InlineEdit" && addressEquals(address, pendingOperation.address)
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
	const { mutate: enterEditMode } = useEnterEditMode(projectId);
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
					setCursor("applied", branchAddress({ branchRef: response.newRef.fullNameBytes }));
				},
			},
		);
	};

	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});

	const deleteCommit = () => {
		const state = store.getState();
		const subjectCommitIds = projectSlice.selectors.selectAddressChecked(state, projectId, address)
			? projectSlice.selectors.selectCheckedCommitIds(state, projectId)
			: new Set([commit.id]);
		const selectionAfterDiscard = selectAfterDiscardedCommits({
			addressSpace,
			commit: commitAddressV,
			discardedCommitIds: subjectCommitIds,
			headInfoIndex,
		});

		commitDiscard(
			{
				projectId,
				subjectCommitIds: Array.from(subjectCommitIds),
				dryRun: false,
			},
			{
				onSuccess: (response) => {
					let latestSelectionAfterDiscard = selectionAfterDiscard;

					rewrite: if (selectionAfterDiscard?._tag === "Commit") {
						const newId = response.workspace.replacedCommits[selectionAfterDiscard.commitId];
						if (newId === undefined) break rewrite;

						latestSelectionAfterDiscard = commitAddress({
							commitId: newId,
							changeId: selectionAfterDiscard.changeId,
						});
					}

					setCursor("applied", latestSelectionAfterDiscard);
				},
			},
		);
	};

	const cutCommit = () => {
		const state = store.getState();
		const sources = projectSlice.selectors.selectAddressChecked(state, projectId, address)
			? projectSlice.selectors.selectCheckedAddresses(state, projectId)
			: [address];

		startKeyboardTransfer({ sources, kind: "move" });
		focusScope("sidebar");
	};

	const copyCommit = () => {
		const state = store.getState();
		const sources = projectSlice.selectors.selectAddressChecked(state, projectId, address)
			? projectSlice.selectors.selectCheckedAddresses(state, projectId)
			: [address];
		if (!sources.every((source) => source._tag === "Commit")) return;

		startKeyboardTransfer({ sources, kind: "copy", placement: "above" });
		focusScope("sidebar");
	};

	const uncommitCommit = () => {
		const state = store.getState();
		const subjectCommitIds = projectSlice.selectors.selectAddressChecked(state, projectId, address)
			? Array.from(projectSlice.selectors.selectCheckedCommitIds(state, projectId))
			: [commit.id];

		commitUncommit({
			projectId,
			assignTo: null,
			subjectCommitIds,
			dryRun: false,
		});
	};

	const startEditing = () => {
		startInlineEdit(address);
	};

	const endEditing = () => {
		dispatch(projectSlice.actions.clearPendingOperation({ projectId }));
		setCursor("applied", address);
		focusScope("sidebar");
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
			accelerator: toElectronAccelerator(sidebarHotkeys.rewordCommit.hotkey),
			onSelect: startEditing,
		}),
		nativeMenuItem({
			label: "Amend Commit",
			accelerator: toElectronAccelerator(changesHotkeys.amendCommit.hotkey),
			enabled: noOperationPending && canAmendCommit,
			onSelect: amendCommit,
		}),
		nativeMenuItem({
			label: "Edit Commit",
			enabled: noOperationPending && stackId !== null,
			onSelect: () => {
				if (stackId !== null) enterEditMode({ projectId, commitId: commit.id, stackId });
			},
		}),
		nativeMenuItem({
			label: "Copy Commit",
			onSelect: copyCommit,
			accelerator: toElectronAccelerator(sidebarHotkeys.copy.hotkey),
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
			accelerator: toElectronAccelerator(sidebarHotkeys.openCommitInBrowser.hotkey),
			onSelect: openCommitInBrowser,
		}),
		insertBlankCommitMenuItem(insertBlankCommit, "above"),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Create Branch",
			submenu: [
				nativeMenuItem({
					label: "Above",
					accelerator: toElectronAccelerator(sidebarHotkeys.createDependentBranchAbove.hotkey),
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
			accelerator: toElectronAccelerator(sidebarHotkeys.deleteCommit.hotkey),
			onSelect: deleteCommit,
		}),
		nativeMenuItem({
			label: "Uncommit",
			enabled: !isCommitUncommitPending,
			accelerator: toElectronAccelerator(sidebarHotkeys.uncommitCommit.hotkey),
			onSelect: uncommitCommit,
		}),
	];

	return (
		<ItemRow
			{...restProps}
			address={address}
			isChecked={isChecked}
			isHighlighted={isDependency}
			onDoubleClick={noOperationPending ? startEditing : undefined}
			onShiftSelect={
				noOperationPending && canCheck
					? () => checkCommit({ commitId: commit.id, shiftKey: true })
					: undefined
			}
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
						disabled={!noOperationPending || !canCheck}
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
							<Tooltip.Popup
								render={<TooltipPopup kbd={sidebarHotkeys.checkCommit.hotkey} kbdScope="sidebar" />}
							>
								{sidebarHotkeys.checkCommit.meta.name}
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
						const caretPosition = firstNewline !== -1 ? firstNewline : el.value.length;
						el.setSelectionRange(caretPosition, caretPosition);
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

			{noOperationPending && (
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
