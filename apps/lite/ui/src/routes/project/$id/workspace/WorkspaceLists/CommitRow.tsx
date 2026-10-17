import { useAddressSpace } from "./context.tsx";
import { startKeyboardTransfer, setCursor, startInlineEdit } from "#ui/use-cursor.ts";
import {
	useBranchCreate,
	useBranchNameSegment,
	useCommitDiscard,
	useCommitInsertBlank,
	useCommitReword,
	useCommitUncommit,
	useEnterEditMode,
} from "#ui/api/mutations.ts";
import { forgeInfoOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { GraphSegment, type GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { Tooltip } from "@gitbutler/ui-react/Tooltip.tsx";
import { commitBody, commitForgeUrl, commitIsDiverged, commitTitle } from "#ui/commit.ts";
import { addErrorToast } from "#ui/errors.ts";
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
import { Toast, Toolbar } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { type ComponentProps, type FC, useId, useOptimistic, useTransition } from "react";
import { RowCheckbox, RowToolbar } from "../Row.tsx";
import { getRowButtonClassName } from "../Row-utils.ts";
import { InlineEditor } from "./InlineEditor.tsx";
import { insertBlankCommitMenuItem } from "./insertBlankCommitMenuItem.ts";
import { ItemRow } from "./ItemRow.tsx";
import { selectAfterDiscardedCommits } from "./selectAfterDiscardedCommit.ts";
import styles from "./CommitRow.module.css";
import { CommitRowContent } from "../CommitRowContent.tsx";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";

export const CommitRow: FC<
	{
		commit: Commit;
		projectId: string;
		/** `null` on a stack without an id, where edit mode cannot be offered. */
		stackId: string | null;
		checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
		amendCommit: () => void;
		canAmendCommit: boolean;
		scrollSelectedIntoView?: boolean;
		/** The rail below the commit's circle: the next icon's colour, or plain under a card's last. */
		below?: GraphSegmentStatus;
		/** Columns of the main line running behind the row, left of its rail. */
		behind?: number;
		/**
		 * The linked worktree whose lane the commit is drawn in. Such a commit is
		 * outside the workspace, so the actions that rewrite it stay off.
		 */
		worktree?: string;
		/** Whether `worktree` has its `HEAD` detached at this commit. */
		isDetachedHead?: boolean;
	} & ComponentProps<"div">
> = ({
	commit,
	projectId,
	stackId,
	checkCommit,
	amendCommit,
	canAmendCommit,
	worktree,
	isDetachedHead = false,
	below,
	behind,
	...restProps
}) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const mforgeUrl = forgeInfo && commitForgeUrl(commit, forgeInfo);
	const commitAddressV: CommitAddress = {
		commitId: commit.id,
		changeId: commit.changeId,
	};
	const address = commitAddress(commitAddressV);

	const descriptionId = useId();
	const inWorkspace = worktree === undefined;
	const canCheck =
		useAppSelector((state) => projectSlice.selectors.selectCanCheckCommits(state, projectId)) &&
		inWorkspace;
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
	const checkDisabled = !noOperationPending || !canCheck;
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

	const { mutate: commitInsertBlank } = useCommitInsertBlank();
	const { isPending: isCommitDiscardPending, mutate: commitDiscard } = useCommitDiscard();
	const { isPending: isCommitUncommitPending, mutate: commitUncommit } = useCommitUncommit();
	const { mutateAsync: commitReword } = useCommitReword();
	const { mutate: enterEditMode } = useEnterEditMode(projectId);
	const { mutate: branchCreate } = useBranchCreate();
	const { isPending: isBranchNameSegmentPending, mutate: branchNameSegment } =
		useBranchNameSegment();

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

	const nameDetachedHead = (worktree: string) => {
		branchNameSegment(
			{
				projectId,
				checkout: { type: "worktree", subject: worktree },
				commitId: commit.id,
				newRef: null,
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
				addErrorToast(toastManager, {
					title: "Failed to reword commit",
					error,
				});
			}
		});
	};

	const openCommitInBrowser = async (): Promise<void> => {
		if (!mforgeUrl) return;

		await window.lite.openInWebBrowser(mforgeUrl.url);
	};

	const title = commitTitle(optimisticMessage);
	const body = commitBody(optimisticMessage);

	// Items that only read the commit, shared by every menu.
	const readOnlyMenuItems: Array<NativeMenuItem> = [
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
	];
	const rewordItem = nativeMenuItem({
		label: "Reword Commit",
		enabled: !isCommitMessagePending,
		// Advertising a hotkey defined elsewhere.
		accelerator: toElectronAccelerator(sidebarHotkeys.rewordCommit.hotkey),
		onSelect: startEditing,
	});
	const amendItem = nativeMenuItem({
		label: "Amend Commit",
		accelerator: toElectronAccelerator(changesHotkeys.amendCommit.hotkey),
		enabled: noOperationPending && canAmendCommit,
		onSelect: amendCommit,
	});
	const copyItem = nativeMenuItem({
		label: "Copy Commit",
		onSelect: copyCommit,
		accelerator: toElectronAccelerator(sidebarHotkeys.copy.hotkey),
	});
	const cutItem = nativeMenuItem({
		label: "Cut Commit",
		onSelect: cutCommit,
		accelerator: toElectronAccelerator(selectionOperationHotkeys.cut.hotkey),
	});
	const deleteItem = nativeMenuItem({
		label: "Delete Commit",
		enabled: !isCommitDiscardPending,
		accelerator: toElectronAccelerator(sidebarHotkeys.deleteCommit.hotkey),
		onSelect: deleteCommit,
	});
	const uncommitItem = nativeMenuItem({
		label: "Uncommit",
		enabled: !isCommitUncommitPending,
		accelerator: toElectronAccelerator(sidebarHotkeys.uncommitCommit.hotkey),
		onSelect: uncommitCommit,
	});
	const workspaceMenuItems: Array<NativeMenuItem> = [
		rewordItem,
		amendItem,
		nativeMenuItem({
			label: "Edit Commit",
			enabled: noOperationPending && stackId !== null,
			onSelect: () => {
				if (stackId !== null) enterEditMode({ projectId, commitId: commit.id, stackId });
			},
		}),
		copyItem,
		cutItem,
		nativeMenuSeparator,
		...readOnlyMenuItems,
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
		deleteItem,
		uncommitItem,
	];
	// A worktree's commit takes the same rewrites, bar the two that need the
	// workspace: edit mode runs in its checkout, and branches can't be ordered
	// in worktrees yet. A detached worktree's commit can name its branch.
	const worktreeMenuItems: Array<NativeMenuItem> = [
		rewordItem,
		amendItem,
		copyItem,
		cutItem,
		nativeMenuSeparator,
		...readOnlyMenuItems,
		insertBlankCommitMenuItem(insertBlankCommit, "above"),
		nativeMenuSeparator,
		deleteItem,
		uncommitItem,
		...(worktree !== undefined && isDetachedHead
			? [
					nativeMenuSeparator,
					nativeMenuItem({
						label: "Name Branch",
						enabled: noOperationPending && !isBranchNameSegmentPending,
						onSelect: () => nameDetachedHead(worktree),
					}),
				]
			: []),
	];
	const menuItems = inWorkspace ? workspaceMenuItems : worktreeMenuItems;

	return (
		<ItemRow
			{...restProps}
			address={address}
			aria-describedby={isRewording ? undefined : descriptionId}
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
					below={below}
					behind={behind}
				/>
				<Tooltip
					content={sidebarHotkeys.checkCommit.meta.name}
					kbd={sidebarHotkeys.checkCommit.hotkey}
					kbdScope="sidebar"
					// This gets in the way when the user tries to move their hover to a
					// sibling row.
					disableHoverablePopup
					// A checkbox that can't be checked has no shortcut to advertise.
					disabled={checkDisabled}
				>
					<RowCheckbox
						disabled={checkDisabled}
						aria-label={`Check commit ${title ?? "(no message)"}`}
						checked={isChecked}
						className={styles.checkbox}
						nativeButton
						onCheckedChange={(_checked, { event }) => {
							const shiftKey =
								(event instanceof MouseEvent || event instanceof KeyboardEvent) &&
								event.shiftKey === true;
							checkCommit({ commitId: commit.id, shiftKey });
						}}
					/>
				</Tooltip>
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
				<CommitRowContent
					commit={commitWithOptimisticMessage}
					hasConflicts={commit.hasConflicts}
					descriptionId={descriptionId}
				/>
			)}

			{noOperationPending && (
				<Toolbar.Root aria-label="Commit actions" render={<RowToolbar reserveSpace />}>
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
