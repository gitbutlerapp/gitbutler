import uiStyles from "#ui/components/ui.module.css";
import { useCommitAmend, useCommitCreate } from "#ui/api/mutations.ts";
import { changesInWorktreeQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import {
	changesHotkeys,
	formatForDisplaySorted,
	outlineHotkeys,
	toElectronAccelerator,
} from "#ui/hotkeys.ts";
import { nativeMenuItem, showNativeMenuFromTrigger, type NativeMenuItem } from "#ui/native-menu.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Button, Combobox, Tooltip } from "@base-ui/react";
import type { RelativeTo } from "@gitbutler/but-sdk";
import { useHotkey, useHotkeys } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { type FC, type SubmitEventHandler, useRef, useState } from "react";
import styles from "./CommitForm.module.css";
import { operandEquals, operandIdentityKey, type Operand } from "#ui/operands.ts";

export type CommitTargetComboboxItem = {
	label: string;
	operand: Operand;
};

// oxlint-disable-next-line react/only-export-components -- TODO: move
export const commitMessageInputId = "commit-message-input";

const CommitTargetComboboxPopup: FC = () => (
	<Combobox.Popup className={classes(uiStyles.popup, "text-13", styles.targetPopup)}>
		<Combobox.Input
			aria-label="Search targets"
			placeholder="Search targets..."
			className={styles.targetInput}
		/>
		<Combobox.Empty>
			<div className={styles.targetEmpty}>No targets found.</div>
		</Combobox.Empty>
		<Combobox.List className={styles.targetList}>
			{(item: CommitTargetComboboxItem) => (
				<Combobox.Item
					key={operandIdentityKey(item.operand)}
					value={item}
					className={styles.targetItem}
				>
					{item.label}
				</Combobox.Item>
			)}
		</Combobox.List>
	</Combobox.Popup>
);

export const CommitForm: FC<{
	projectId: string;
	commitTarget: CommitTargetComboboxItem | null;
	targetComboboxItems: Array<CommitTargetComboboxItem>;
}> = ({ projectId, commitTarget, targetComboboxItems }) => {
	const dispatch = useAppDispatch();
	const { isPending: isCommitCreatePending, mutate: commitCreate } = useCommitCreate({
		projectId,
	});
	const { isPending: isCommitAmendPending, mutate: commitAmend } = useCommitAmend({
		projectId,
	});

	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const commitTextareaRef = useRef<HTMLTextAreaElement | null>(null);

	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);

	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const isCommitOrAmendPending = isCommitCreatePending || isCommitAmendPending;
	const canCommitOrAmendBase = isDefaultMode && commitTarget !== null && !isCommitOrAmendPending;
	const canCommit = canCommitOrAmendBase;
	const canAmend =
		canCommitOrAmendBase &&
		worktreeChanges &&
		worktreeChanges.changes.length > 0 &&
		headInfoIndex &&
		((commitTarget.operand._tag === "Branch" &&
			!!headInfoIndex.branchContextByRefBytes(commitTarget.operand.branchRef)) ||
			(commitTarget.operand._tag === "Commit" &&
				!!headInfoIndex.commitContextById(commitTarget.operand.changeId)));

	const [open, setOpen] = useState(false);

	const selectBranch = (option: CommitTargetComboboxItem | null) => {
		dispatch(
			projectSlice.actions.setCommitTarget({
				projectId,
				commitTarget: option?.operand ?? null,
			}),
		);
		setOpen(false);
	};

	const createCommit = () => {
		let relativeTo: RelativeTo | null = null;
		if (commitTarget?.operand._tag === "Commit") {
			const subject = headInfoIndex?.commitContextById(commitTarget.operand.changeId)?.commit.id;
			if (subject === undefined) throw new Error("Could not find commit subject");
			relativeTo = { type: "commit", subject };
		} else if (commitTarget?.operand._tag === "Branch") {
			relativeTo = { type: "referenceBytes", subject: commitTarget.operand.branchRef };
		}

		if (!relativeTo) throw new Error("Invalid commit target");

		commitCreate(
			{
				message: commitTextareaRef.current?.value ?? "",
				relativeTo,
			},
			{
				onSuccess: (response) => {
					if (response.newCommit !== null && commitTextareaRef.current)
						commitTextareaRef.current.value = "";
				},
			},
		);
	};

	const amendCommit = () => {
		let commitId;
		if (commitTarget?.operand._tag === "Commit") {
			commitId = headInfoIndex?.commitContextById(commitTarget.operand.changeId)?.commit.id;
		} else if (commitTarget?.operand._tag === "Branch") {
			commitId = headInfoIndex?.branchContextByRefBytes(commitTarget.operand.branchRef)?.segment
				.commits[0]?.id;
		}

		if (commitId === undefined) throw new Error("Could not find commit to amend into");

		commitAmend({ commitId });
	};
	const submit: SubmitEventHandler = (event) => {
		event.preventDefault();

		createCommit();
	};
	const commitMenuItems: Array<NativeMenuItem> = [
		// oxlint-disable-next-line react-hooks-js/refs -- False positive. Ref is only accessed in `onSelect` event handler.
		nativeMenuItem({
			label: "Commit",
			enabled: canCommit,
			accelerator: toElectronAccelerator(changesHotkeys.commit.hotkey),
			onSelect: createCommit,
		}),
		nativeMenuItem({
			label: "Amend Commit",
			enabled: canAmend,
			accelerator: toElectronAccelerator(changesHotkeys.amendCommit.hotkey),
			onSelect: amendCommit,
		}),
	];

	useHotkeys([
		{
			hotkey: changesHotkeys.selectCommitTarget.hotkey,
			callback: () => setOpen(true),
			options: {
				conflictBehavior: "allow",
				enabled: isDefaultMode && !isCommitOrAmendPending,
			},
		},
		{
			hotkey: changesHotkeys.commit.hotkey,
			callback: createCommit,
			options: {
				conflictBehavior: "allow",
				enabled: canCommit,
				meta: changesHotkeys.commit.meta,
			},
		},
		{
			hotkey: changesHotkeys.amendCommit.hotkey,
			callback: amendCommit,
			options: {
				conflictBehavior: "allow",
				enabled: canAmend,
				meta: changesHotkeys.amendCommit.meta,
			},
		},
	]);

	useHotkey("Escape", () => focusSelectionScope("uncommitted-files"), {
		target: commitTextareaRef,
		conflictBehavior: "allow",
	});

	const commitTextareaLabel = `Compose commit message ${formatForDisplaySorted(
		outlineHotkeys.composeCommitMessage.hotkey,
	)}`;

	return (
		<form onSubmit={submit} className={styles.form}>
			<textarea
				id={commitMessageInputId}
				ref={commitTextareaRef}
				aria-label={commitTextareaLabel}
				disabled={!isDefaultMode}
				readOnly={isCommitOrAmendPending}
				placeholder={commitTextareaLabel}
				className={classes("text-13", "text-body", styles.textarea)}
			/>

			<div className={styles.footer}>
				<Combobox.Root<CommitTargetComboboxItem>
					items={targetComboboxItems}
					open={open}
					onOpenChange={setOpen}
					// Note `undefined` means uncontrolled.
					value={commitTarget ?? null}
					onValueChange={selectBranch}
					itemToStringLabel={(x) => x.label}
					itemToStringValue={(x) => operandIdentityKey(x.operand)}
					isItemEqualToValue={(a, b) => operandEquals(a.operand, b.operand)}
					autoHighlight
					disabled={!isDefaultMode || isCommitOrAmendPending}
				>
					<Tooltip.Root>
						<Combobox.Trigger
							className={classes("text-13 text-semibold", styles.targetTrigger)}
							aria-label="Select commit target"
							// We pass `disabled` here because we want to disable the button, not
							// the tooltip. Other props should be passed above.
							render={<Button focusableWhenDisabled render={<Tooltip.Trigger />} />}
						>
							<Icon name="bullseye" size={14} />
							<span className={styles.targetTriggerLabel}>
								<Combobox.Value placeholder="Select commit target" />
							</span>
						</Combobox.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup
									render={<TooltipPopup kbd={changesHotkeys.selectCommitTarget.hotkey} />}
								>
									Select commit target
								</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
					<Combobox.Portal>
						<Combobox.Positioner align="start" sideOffset={4}>
							<CommitTargetComboboxPopup />
						</Combobox.Positioner>
					</Combobox.Portal>
				</Combobox.Root>

				<div className={styles.dropdownButton}>
					<Tooltip.Root>
						<Tooltip.Trigger
							className={getButtonClassName({ variant: "pop" })}
							// We pass `disabled` here because we want to disable the button, not
							// the tooltip. Other props should be passed above.
							render={<Button focusableWhenDisabled type="submit" disabled={!canCommit} />}
						>
							Commit
						</Tooltip.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup render={<TooltipPopup kbd={changesHotkeys.commit.hotkey} />}>
									{changesHotkeys.commit.meta.name}
								</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
					<div aria-hidden className={styles.dropdownButtonSeparator} />
					<Button
						focusableWhenDisabled
						disabled={!(canAmend || canCommit)}
						aria-label="Commit options"
						className={getButtonClassName({ variant: "pop", iconOnly: true })}
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, commitMenuItems);
						}}
					>
						<Icon name="chevron-down" />
					</Button>
				</div>
			</div>
		</form>
	);
};
