import uiStyles from "#ui/components/ui.module.css";
import { useCommitAmend, useCommitCreate } from "#ui/api/mutations.ts";
import { changesInWorktreeQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { relativeToEquals, relativeToKey } from "#ui/api/relative-to.ts";
import { getHeadInfoIndex, resolveRelativeTo } from "#ui/api/ref-info.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { Kbd } from "#ui/components/Kbd.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import {
	changesHotkeys,
	formatForDisplaySorted,
	outlineHotkeys,
	toElectronAccelerator,
} from "#ui/hotkeys.ts";
import { nativeMenuItem, showNativeMenuFromTrigger, type NativeMenuItem } from "#ui/native-menu.ts";
import { uncommittedChangesOperand } from "#ui/operands.ts";
import { projectActions, selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Button, Tooltip } from "@base-ui/react";
import { Combobox } from "@base-ui/react/combobox";
import type { RelativeTo } from "@gitbutler/but-sdk";
import { useHotkey, useHotkeys } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { type FC, type SubmitEventHandler, useRef, useState } from "react";
import styles from "./CommitForm.module.css";

export type CommitTargetComboboxItem = {
	label: string;
	relativeTo: RelativeTo;
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
					key={relativeToKey(item.relativeTo)}
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
	const commitCreateMutation = useCommitCreate({ projectId });
	const commitAmendMutation = useCommitAmend({ projectId });

	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const operand = uncommittedChangesOperand;
	const commitTextareaRef = useRef<HTMLTextAreaElement | null>(null);

	const isDefaultMode = useAppSelector(
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);

	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const isCommitOrAmendPending = commitCreateMutation.isPending || commitAmendMutation.isPending;
	const canCommitOrAmendBase = isDefaultMode && commitTarget !== null && !isCommitOrAmendPending;
	const canCommit = canCommitOrAmendBase;
	const canAmend =
		canCommitOrAmendBase &&
		worktreeChanges &&
		worktreeChanges.changes.length > 0 &&
		headInfoIndex &&
		resolveRelativeTo({ headInfoIndex, relativeTo: commitTarget.relativeTo }) !== null;

	const [open, setOpen] = useState(false);

	const selectBranch = (option: CommitTargetComboboxItem | null) => {
		dispatch(
			projectActions.setCommitTarget({
				projectId,
				commitTarget: option?.relativeTo ?? null,
			}),
		);
		setOpen(false);
	};

	const selectChanges = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
	};
	const createCommit = () => {
		if (!commitTarget) return;

		commitCreateMutation.mutate(
			{
				message: commitTextareaRef.current?.value ?? "",
				relativeTo: commitTarget.relativeTo,
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
		if (!commitTarget || !headInfoIndex) return;

		const commitId = resolveRelativeTo({
			headInfoIndex,
			relativeTo: commitTarget.relativeTo,
		});
		if (commitId === null) throw new Error("No commit to amend.");

		commitAmendMutation.mutate({ commitId });
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

	useHotkey("Escape", () => focusSelectionScope("outline"), {
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
				onFocus={selectChanges}
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
					itemToStringValue={(x) => relativeToKey(x.relativeTo)}
					isItemEqualToValue={(a, b) => relativeToEquals(a.relativeTo, b.relativeTo)}
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

				{/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Grouping keeps the split submit control together. */}
				<div role="group" className={styles.dropdownButton}>
					<Tooltip.Root>
						<Tooltip.Trigger
							className={getButtonClassName({ variant: "pop" })}
							// We pass `disabled` here because we want to disable the button, not
							// the tooltip. Other props should be passed above.
							render={<Button focusableWhenDisabled type="submit" disabled={!canCommit} />}
						>
							Commit
							<Kbd hotkey={changesHotkeys.commit.hotkey} />
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
