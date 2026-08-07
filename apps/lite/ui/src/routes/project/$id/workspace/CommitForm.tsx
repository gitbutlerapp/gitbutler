import uiStyles from "#ui/components/ui.module.css";
import { commitAmendMutationKey, useCommitCreate } from "#ui/api/mutations.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex, resolveRelativeTo } from "#ui/api/ref-info.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { Kbd } from "#ui/components/Kbd.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { draftCommitMessageQueryOptions, usePersistDraftCommitMessage } from "#ui/draft.ts";
import { changesHotkeys, outlineHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import { nativeMenuItem, showNativeMenuFromTrigger, type NativeMenuItem } from "#ui/native-menu.ts";
import { operandEquals, operandIdentityKey, type Operand } from "#ui/operands.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { Button, Combobox, Tooltip } from "@base-ui/react";
import type { InsertSide, RelativeTo, WorktreeChanges } from "@gitbutler/but-sdk";
import { useHotkey, useHotkeys } from "@tanstack/react-hotkeys";
import { useIsMutating, useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import {
	type FC,
	type ReactNode,
	type RefCallback,
	type SubmitEventHandler,
	useRef,
	useState,
} from "react";
import styles from "./CommitForm.module.css";

export type CommitTargetComboboxItem = {
	label: string;
	operand: Extract<Operand, { _tag: "Branch" | "Commit" }>;
	relativeTo: RelativeTo;
};

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

/**
 * Wires up the commit target combobox. The trigger is passed as children so the
 * same picker can be rendered both in the expanded form's footer and next to
 * the collapsed "Start commit" button.
 */
const CommitTargetCombobox: FC<{
	items: Array<CommitTargetComboboxItem>;
	value: CommitTargetComboboxItem | null;
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onValueChange: (item: CommitTargetComboboxItem | null) => void;
	disabled: boolean;
	children: ReactNode;
}> = ({ items, value, open, onOpenChange, onValueChange, disabled, children }) => (
	<Combobox.Root<CommitTargetComboboxItem>
		items={items}
		open={open}
		onOpenChange={onOpenChange}
		// Note `undefined` means uncontrolled.
		value={value}
		onValueChange={onValueChange}
		itemToStringLabel={(x) => x.label}
		itemToStringValue={(x) => operandIdentityKey(x.operand)}
		isItemEqualToValue={(a, b) => operandEquals(a.operand, b.operand)}
		autoHighlight
		disabled={disabled}
	>
		{children}
		<Combobox.Portal>
			<Combobox.Positioner align="start" sideOffset={4}>
				<CommitTargetComboboxPopup />
			</Combobox.Positioner>
		</Combobox.Portal>
	</Combobox.Root>
);

export const CommitForm: FC<{
	projectId: string;
	commitTarget: CommitTargetComboboxItem | null;
	targetComboboxItems: Array<CommitTargetComboboxItem>;
	startCommitButtonId: string;
	commitMessageInputId: string;
	onAmendCommit: (commitId: string) => void;
	canAmendCommit: boolean;
	worktreeChanges: WorktreeChanges | undefined;
	className?: string;
}> = ({
	projectId,
	commitTarget,
	targetComboboxItems,
	startCommitButtonId,
	commitMessageInputId,
	onAmendCommit,
	canAmendCommit,
	worktreeChanges,
	className,
}) => {
	const dispatch = useAppDispatch();
	const store = useAppStore();
	const { isPending: isCommitCreatePending, mutate: commitCreate } = useCommitCreate();

	const commitTextareaRef = useRef<HTMLTextAreaElement | null>(null);
	const formRef = useRef<HTMLFormElement | null>(null);

	const { data: draftMessage } = useQuery(draftCommitMessageQueryOptions(projectId));
	const { mutate: persistDraftMessage } = usePersistDraftCommitMessage();

	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);

	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const isAmendCommitPending = useIsMutating({ mutationKey: commitAmendMutationKey }) > 0;
	const isCommitOrAmendPending = isCommitCreatePending || isAmendCommitPending;

	const [open, setOpen] = useState(false);
	const [isExpanded, setIsExpanded] = useState(false);
	const [commitLabelHidden, setCommitLabelHidden] = useState(false);

	// Track whether the container query hides the label, including while resizing.
	// This is a ref callback rather than a mount effect because the form is
	// conditionally rendered, so on mount the label doesn't exist yet. The label is
	// a flex item, so `display: none` zeroes its box and observing it is enough to
	// tell — no need to read computed styles.
	const observeCommitLabel: RefCallback<HTMLSpanElement> = (label) => {
		if (label === null) return;

		// A zero-size box matches the initial reported size, so it doesn't trigger
		// an observation. Measure up front to catch a label that mounts hidden.
		setCommitLabelHidden(label.offsetWidth === 0);

		const observer = new ResizeObserver((entries) => {
			for (const entry of entries) setCommitLabelHidden(entry.contentRect.width === 0);
		});
		observer.observe(label);
		return () => observer.disconnect();
	};

	const canCommitOrAmendBase = isDefaultMode && commitTarget !== null && !isCommitOrAmendPending;
	const canCommit = canCommitOrAmendBase;
	const amendTargetCommitId =
		commitTarget && headInfoIndex
			? resolveRelativeTo({ headInfoIndex, relativeTo: commitTarget.relativeTo })
			: null;
	const canAmend = canCommitOrAmendBase && canAmendCommit && amendTargetCommitId !== null;

	const selectBranch = (option: CommitTargetComboboxItem | null) => {
		if (option)
			dispatch(projectSlice.actions.selectOutline({ projectId, selection: option.operand }));
		setOpen(false);
	};

	const createCommit = () => {
		if (!commitTarget || !worktreeChanges) return;

		const checkedUncommittedFilePaths = projectSlice.selectors.selectCheckedUncommittedFilePaths(
			store.getState(),
			projectId,
		);
		commitCreate(
			{
				projectId,
				message: commitTextareaRef.current?.value ?? draftMessage ?? "",
				relativeTo: commitTarget.relativeTo,
				changes: worktreeChanges.changes.flatMap((change) =>
					checkedUncommittedFilePaths.size === 0 || checkedUncommittedFilePaths.has(change.path)
						? [createDiffSpec(change, [])]
						: [],
				),
				changesSource: { type: "head" },
				side: Match.value(commitTarget.relativeTo).pipe(
					Match.withReturnType<InsertSide>(),
					Match.when({ type: "commit" }, () => "above"),
					Match.when({ type: "reference" }, () => "below"),
					Match.when({ type: "referenceBytes" }, () => "below"),
					Match.exhaustive,
				),
				dryRun: false,
			},
			{
				onSuccess: (response) => {
					if (response.newCommit === null) return;

					if (commitTextareaRef.current) commitTextareaRef.current.value = "";

					persistDraftMessage({ projectId, message: "" });
				},
			},
		);
	};

	const amendCommit = () => {
		if (amendTargetCommitId === null) throw new Error("No commit to amend.");

		onAmendCommit(amendTargetCommitId);
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

	// Note we deliberately don't scope this hotkey with `target` refs. The form
	// is conditionally rendered, so the refs are `null` on mount, and the hook
	// would never register the listener.
	useHotkey(
		"Escape",
		() => {
			const form = formRef.current;
			if (!form || !form.contains(document.activeElement)) return;

			// Persist the draft before the textarea unmounts.
			persistDraftMessage({ projectId, message: commitTextareaRef.current?.value ?? "" });
			setIsExpanded(false);
			setOpen(false);
			focusSelectionScope("uncommitted-files");
		},
		{
			conflictBehavior: "allow",
			enabled: isExpanded,
		},
	);

	const commitTextareaLabel = "Compose commit message";

	if (!isExpanded) {
		return (
			<div className={classes(styles.startCommitRow, className)}>
				<CommitTargetCombobox
					items={targetComboboxItems}
					value={commitTarget ?? null}
					open={open}
					onOpenChange={setOpen}
					onValueChange={selectBranch}
					disabled={!isDefaultMode || isCommitOrAmendPending}
				>
					<Tooltip.Root>
						<Combobox.Trigger
							className={classes(
								getButtonClassName({ variant: "outline" }),
								styles.collapsedTargetTrigger,
							)}
							aria-label="Select commit target"
							render={<Button focusableWhenDisabled render={<Tooltip.Trigger />} />}
						>
							<Icon name="bullseye" size={14} />
							<Icon
								name={commitTarget?.operand._tag === "Commit" ? "commit" : "branch"}
								size={14}
							/>
						</Combobox.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup
									render={<TooltipPopup kbd={changesHotkeys.selectCommitTarget.hotkey} />}
								>
									{commitTarget ? (
										<span className={styles.tooltipTarget}>
											<span className={styles.tooltipTargetLabel}>Target:</span>
											<span className={styles.tooltipTargetName}>{commitTarget.label}</span>
										</span>
									) : (
										"Select commit target"
									)}
								</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				</CommitTargetCombobox>

				{/* Amend ignores the message, so its affordance belongs here rather than
				    behind the message composer. Mirrors the hotkeys, which are registered
				    regardless of whether the form is expanded. */}
				<div className={classes(styles.dropdownButton, styles.startCommitSplit)}>
					<Button
						className={classes(getButtonClassName({ variant: "pop" }), styles.startCommitButton)}
						id={startCommitButtonId}
						onClick={() => setIsExpanded(true)}
						focusableWhenDisabled
						disabled={!isDefaultMode}
					>
						Start commit
						<Kbd hotkey={outlineHotkeys.composeCommitMessage.hotkey} variant="button" />
					</Button>
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
		);
	}

	return (
		// oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- Used for persistence, not UI per se.
		<form
			ref={formRef}
			onSubmit={submit}
			onBlur={(e) => {
				const next = e.relatedTarget;
				if (next instanceof Node && e.currentTarget.contains(next)) return;
				persistDraftMessage({ projectId, message: commitTextareaRef.current?.value ?? "" });
			}}
			className={classes(styles.form, className)}
		>
			<textarea
				// The form is only rendered expanded after interacting with the
				// "Start commit" trigger, so focusing the input is expected.
				// oxlint-disable-next-line jsx_a11y/no-autofocus
				autoFocus
				id={commitMessageInputId}
				ref={(el) => {
					commitTextareaRef.current = el;
					// Place the caret at the end of the restored draft message.
					el?.setSelectionRange(el.value.length, el.value.length);
				}}
				aria-label={commitTextareaLabel}
				disabled={!isDefaultMode}
				readOnly={isCommitOrAmendPending}
				placeholder={commitTextareaLabel}
				defaultValue={draftMessage ?? ""}
				className={classes("text-13", "text-body", styles.textarea, uiStyles.overlayScrollbar)}
			/>

			<div className={styles.footer}>
				<CommitTargetCombobox
					items={targetComboboxItems}
					value={commitTarget ?? null}
					open={open}
					onOpenChange={setOpen}
					onValueChange={selectBranch}
					disabled={!isDefaultMode || isCommitOrAmendPending}
				>
					<Tooltip.Root>
						<Combobox.Trigger
							className={classes("text-13 text-semibold", styles.targetTrigger)}
							aria-label="Select commit target"
							render={<Button focusableWhenDisabled render={<Tooltip.Trigger />} />}
						>
							<Icon
								name={commitTarget?.operand._tag === "Commit" ? "commit" : "branch"}
								size={14}
							/>
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
				</CommitTargetCombobox>

				<div className={styles.commitActions}>
					<Tooltip.Root>
						<Tooltip.Trigger
							className={getButtonClassName({ variant: "outline" })}
							onClick={() => {
								// Persist the draft before the textarea unmounts.
								persistDraftMessage({
									projectId,
									message: commitTextareaRef.current?.value ?? "",
								});
								setIsExpanded(false);
								setOpen(false);
								focusSelectionScope("uncommitted-files");
							}}
							render={
								<Button focusableWhenDisabled disabled={isCommitOrAmendPending} type="button" />
							}
						>
							Cancel
						</Tooltip.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup render={<TooltipPopup kbd="Escape" />}>Cancel</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>

					{/* The tooltip is redundant while the label is visible. */}
					<Tooltip.Root disabled={!commitLabelHidden}>
						<Tooltip.Trigger
							aria-label="Commit"
							className={getButtonClassName({ variant: "pop" })}
							render={<Button focusableWhenDisabled type="submit" disabled={!canCommit} />}
						>
							<span ref={observeCommitLabel} className={styles.commitButtonLabel}>
								Commit
							</span>
							<Kbd hotkey={changesHotkeys.commit.hotkey} variant="button" />
						</Tooltip.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup render={<TooltipPopup kbd={changesHotkeys.commit.hotkey} />}>
									Commit
								</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				</div>
			</div>
		</form>
	);
};
