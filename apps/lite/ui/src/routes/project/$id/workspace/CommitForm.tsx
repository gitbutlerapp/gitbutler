import { Popup, PopupItem, PopupSearch } from "#ui/components/Popup.tsx";
import { setCursor } from "#ui/use-cursor.ts";
import { useBranchCreate, useCommitCreate, useGenerateCommitMessage } from "#ui/api/mutations.ts";
import {
	aiConfigurationQueryOptions,
	branchCannedNameQueryOptions,
	headInfoQueryOptions,
	operatingModeQueryOptions,
} from "#ui/api/queries.ts";
import { getHeadInfoIndex, resolveRelativeTo } from "#ui/api/ref-info.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { DropdownButton } from "#ui/components/DropdownButton.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import type { IconName } from "#ui/components/iconNames.ts";
import { Kbd } from "#ui/components/Kbd.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import {
	changesSelectedForCommit,
	commitMessageGenerationButtonState,
} from "#ui/commit-message-generation.ts";
import { draftCommitMessageQueryOptions, usePersistDraftCommitMessage } from "#ui/commit.ts";
import { changesHotkeys, sidebarHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import { nativeMenuItem, showNativeMenuFromTrigger, type NativeMenuItem } from "#ui/native-menu.ts";
import { addressIdentityKey, type Address } from "#ui/addresses.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { COMMIT_FORM_ATTRIBUTE } from "#ui/routes/project/$id/workspace/commitFormEvent.ts";
import { NO_DRAG_ATTRIBUTE } from "#ui/routes/project/$id/workspace/DragData.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { projectAiSettingsQueryOptions } from "#ui/project-ai-settings.ts";
import { focusScope } from "#ui/focus-scopes.ts";
import { useAppSelector, useAppStore } from "#ui/store.ts";
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
	address: Extract<Address, { _tag: "Branch" | "Commit" }>;
	relativeTo: RelativeTo;
};

/**
 * The picker's one row that is not a target: choosing it has the commit create
 * a branch on submit and land there. It has no `Address` because the branch
 * does not exist until then, and that absence is what tells the row apart.
 */
const newBranchItem = { label: "New branch", address: null } as const;
/** Stands in for a missing target where a target's identity key is expected. */
const noTargetKey = "no-target";
type CommitTargetPickerItem = CommitTargetComboboxItem | typeof newBranchItem;

const pickerItemKey = (item: CommitTargetPickerItem) =>
	item.address === null ? "new-branch" : addressIdentityKey(item.address);
const pickerItemIcon = (item: CommitTargetPickerItem | null): IconName =>
	item === null
		? "branch"
		: item.address === null
			? "plus"
			: item.address._tag === "Commit"
				? "commit"
				: "branch";

const CommitTargetComboboxPopup: FC<{ current: CommitTargetPickerItem | null }> = ({ current }) => (
	// Base UI's combobox owns its own popup part, so this cannot go through `Dropdown` — `Popup`
	// dresses the combobox's own popup instead, and it opens the way every other dropdown does.
	<Popup anchored className={styles.targetPopup} render={<Combobox.Popup />}>
		<PopupSearch
			aria-label="Search targets"
			placeholder="Search targets..."
			render={<Combobox.Input />}
		/>
		<Combobox.Empty>
			<div className={classes("text-13", styles.targetEmpty)}>No targets found.</div>
		</Combobox.Empty>
		<Combobox.List className={styles.targetList}>
			{(item: CommitTargetPickerItem) => (
				<PopupItem
					key={pickerItemKey(item)}
					icon={pickerItemIcon(item)}
					// The bullseye marks where a commit would land, so it rides only the row that is
					// the target now — the rest of the list is where it could go instead.
					trailing={
						current !== null && pickerItemKey(item) === pickerItemKey(current)
							? "bullseye"
							: undefined
					}
					render={<Combobox.Item value={item} />}
				>
					{item.label}
				</PopupItem>
			)}
		</Combobox.List>
	</Popup>
);

/**
 * Wires up the commit target combobox. The trigger is passed as children so the
 * same picker can be rendered both in the expanded form's footer and next to
 * the collapsed "Start commit" button.
 */
const CommitTargetCombobox: FC<{
	items: Array<CommitTargetPickerItem>;
	value: CommitTargetPickerItem | null;
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onValueChange: (item: CommitTargetPickerItem | null) => void;
	disabled: boolean;
	children: ReactNode;
}> = ({ items, value, open, onOpenChange, onValueChange, disabled, children }) => (
	<Combobox.Root<CommitTargetPickerItem>
		items={items}
		open={open}
		onOpenChange={onOpenChange}
		// Note `undefined` means uncontrolled.
		value={value}
		onValueChange={onValueChange}
		itemToStringLabel={(x) => x.label}
		itemToStringValue={pickerItemKey}
		isItemEqualToValue={(a, b) => pickerItemKey(a) === pickerItemKey(b)}
		autoHighlight
		disabled={disabled}
	>
		{children}
		<Combobox.Portal>
			<Combobox.Positioner align="start" sideOffset={4}>
				<CommitTargetComboboxPopup current={value} />
			</Combobox.Positioner>
		</Combobox.Portal>
	</Combobox.Root>
);

export const CommitForm: FC<{
	projectId: string;
	commitTarget: CommitTargetComboboxItem | null;
	targetComboboxItems: Array<CommitTargetComboboxItem>;
	/**
	 * Whether the workspace holds no branch to commit onto. Committing is still
	 * allowed — the branch is created on submit — so this is deliberately kept
	 * apart from `commitTarget`, whose items carry an `Address` that drives the
	 * applied selection and which a branch that doesn't exist yet cannot have.
	 */
	hasNoBranches: boolean;
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
	hasNoBranches,
	startCommitButtonId,
	commitMessageInputId,
	onAmendCommit,
	canAmendCommit,
	worktreeChanges,
	className,
}) => {
	const store = useAppStore();
	const { isPending: isCommitCreatePending, mutate: commitCreate } = useCommitCreate();
	const { isPending: isBranchCreatePending, mutate: branchCreate } = useBranchCreate();
	const { isPending: isGenerating, mutate: generateMessage } = useGenerateCommitMessage();

	const commitTextareaRef = useRef<HTMLTextAreaElement | null>(null);
	const formRef = useRef<HTMLFormElement | null>(null);

	const { data: draftMessage } = useQuery(draftCommitMessageQueryOptions(projectId));
	// Same gate as the sidebar's "New Branch in Workspace": outside an open
	// workspace a new lane is not what starting a branch means.
	const { data: isOpenWorkspace = false } = useQuery({
		...operatingModeQueryOptions(projectId),
		select: (headAndMode) => headAndMode.operatingMode.type === "OpenWorkspace",
	});
	const { mutate: persistDraftMessage } = usePersistDraftCommitMessage();
	const { data: isAiConfigured = false } = useQuery({
		...aiConfigurationQueryOptions,
		select: (configuration) => configuration.isConfigured,
	});
	const { data: isProjectAiEnabled = false } = useQuery({
		...projectAiSettingsQueryOptions(projectId),
		select: (settings) => settings.enabled,
	});
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);

	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const isAmendCommitPending = useIsMutating({ mutationKey: [projectId, "commitAmend"] }) > 0;
	// The branch creation is the first half of a commit here, so it keeps the
	// form read-only for its duration and rules out a double submit.
	const isCommitOrAmendPending =
		isCommitCreatePending || isAmendCommitPending || isBranchCreatePending;

	// The picker's "New branch" choice, remembered against the target it was
	// made over: the applied selection is the commit target, so selecting
	// another branch or commit in the outline supersedes it the way it would any
	// picked target.
	const [newBranchOver, setNewBranchOver] = useState<string | null>(null);
	const currentTargetKey = commitTarget ? addressIdentityKey(commitTarget.address) : noTargetKey;
	const willCreateBranch = hasNoBranches || newBranchOver === currentTargetKey;
	const pickerValue: CommitTargetPickerItem | null = willCreateBranch
		? newBranchItem
		: commitTarget;

	// Only meaningful when the commit creates a branch, and pointless to fetch
	// otherwise: the target then comes from the combobox.
	const { data: cannedBranchName } = useQuery({
		...branchCannedNameQueryOptions(projectId),
		enabled: willCreateBranch,
	});
	const draftBranchLabel = cannedBranchName ?? "New branch";

	const [open, setOpen] = useState(false);
	const [isExpanded, setIsExpanded] = useState(false);
	const [commitLabelHidden, setCommitLabelHidden] = useState(false);
	const generationButton = commitMessageGenerationButtonState({
		enabled: isProjectAiEnabled,
		configured: isAiConfigured,
		busy: isGenerating || isCommitOrAmendPending,
		changeCount: worktreeChanges?.changes.length ?? 0,
	});

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

	const ready = noOperationPending && !isCommitOrAmendPending && !isGenerating;
	// Only used for emphasis, never to gate the action: committing a clean
	// worktree writes an empty commit, which `create_tree` supports deliberately
	// (an empty change list keeps the parent's tree) and people do want.
	const hasWorktreeChanges = (worktreeChanges?.changes.length ?? 0) > 0;
	// A commit that creates its branch needs no target, so it must not be blocked
	// on one. Amending still needs a commit that already exists.
	const canCommit = ready && (commitTarget !== null || willCreateBranch);
	const amendTargetCommitId =
		commitTarget && headInfoIndex
			? resolveRelativeTo({ headInfoIndex, relativeTo: commitTarget.relativeTo })
			: null;
	const canAmend = ready && canAmendCommit && amendTargetCommitId !== null;

	const selectTarget = (option: CommitTargetPickerItem | null) => {
		if (option?.address === null) {
			setNewBranchOver(currentTargetKey);
		} else if (option) {
			setNewBranchOver(null);
			setCursor("applied", option.address);
		}
		setOpen(false);
	};

	const commitOnto = (relativeTo: RelativeTo) => {
		if (!worktreeChanges) return;

		const checkedUncommittedFilePaths = projectSlice.selectors.selectCheckedUncommittedFilePaths(
			store.getState(),
			projectId,
		);
		commitCreate(
			{
				projectId,
				message: commitTextareaRef.current?.value ?? draftMessage ?? "",
				relativeTo,
				changes: worktreeChanges.changes
					.values()
					.filter(
						(change) =>
							checkedUncommittedFilePaths.size === 0 ||
							checkedUncommittedFilePaths.has(change.path),
					)
					.map((change) => createDiffSpec(change, []))
					.toArray(),
				changesSource: { type: "head" },
				side: Match.value(relativeTo).pipe(
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

	const createCommit = () => {
		if (!willCreateBranch) {
			if (commitTarget) commitOnto(commitTarget.relativeTo);
			return;
		}

		// The branch is created first — lazily, so that merely choosing it writes
		// no ref. On failure `useBranchCreate` toasts and no commit is attempted,
		// leaving the form and its draft message untouched.
		if (!worktreeChanges) return;

		branchCreate(
			{ projectId, newRef: null, placement: { type: "independent" } },
			{
				onSuccess: (response) => {
					// The new branch is the target from here on, also for the retry
					// should the commit itself fail.
					setNewBranchOver(null);
					setCursor("applied", {
						_tag: "Branch",
						branchRef: response.newRef.fullNameBytes,
					});
					commitOnto({ type: "referenceBytes", subject: response.newRef.fullNameBytes });
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

	const generateCommitMessage = () => {
		if (!worktreeChanges || isGenerating) return;

		const checkedPaths = projectSlice.selectors.selectCheckedUncommittedFilePaths(
			store.getState(),
			projectId,
		);
		const changes = changesSelectedForCommit(worktreeChanges.changes, checkedPaths);
		if (changes.length === 0) return;

		generateMessage(
			{
				projectId,
				changes,
				previousMessage: commitTextareaRef.current?.value ?? draftMessage ?? "",
				onValue: (value) => {
					if (commitTextareaRef.current) commitTextareaRef.current.value = value;
				},
			},
			{
				onSuccess: (response) => {
					const message = response.trim();
					if (commitTextareaRef.current) commitTextareaRef.current.value = message;
					persistDraftMessage({ projectId, message });
				},
			},
		);
	};
	// Built when the menu opens rather than during render: the callbacks close
	// over the textarea ref, and handing them to a function in render is a ref
	// read as far as React Compiler is concerned, which left this whole
	// component uncompiled.
	const commitMenuItems = (): Array<NativeMenuItem> => [
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
				enabled: noOperationPending && !isCommitOrAmendPending && !hasNoBranches,
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
			focusScope("uncommitted-files");
		},
		{
			conflictBehavior: "allow",
			enabled: isExpanded && !isGenerating,
		},
	);

	const commitTextareaLabel = "Compose commit message";

	// The collapsed row and the expanded footer show the same picker, differing
	// only in how the trigger is dressed. A render function rather than a
	// component, so the picker state needs no threading through props.
	const renderTargetPicker = (trigger: { className: string; iconSize?: number }) => (
		<CommitTargetCombobox
			// The new-branch row goes last, so that `autoHighlight` lands on a target
			// and Enter never creates a branch by accident.
			items={isOpenWorkspace ? [...targetComboboxItems, newBranchItem] : targetComboboxItems}
			value={pickerValue}
			open={open}
			onOpenChange={setOpen}
			onValueChange={selectTarget}
			disabled={!ready || hasNoBranches}
		>
			<Tooltip.Root>
				<Combobox.Trigger
					className={trigger.className}
					aria-label={
						willCreateBranch ? `Will create branch ${draftBranchLabel}` : "Select commit target"
					}
					render={<Button focusableWhenDisabled render={<Tooltip.Trigger />} />}
				>
					<Icon name="bullseye" size={trigger.iconSize} />
					<Icon name={pickerItemIcon(pickerValue)} size={trigger.iconSize} />
				</Combobox.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup
							render={
								<TooltipPopup
									kbd={hasNoBranches ? undefined : changesHotkeys.selectCommitTarget.hotkey}
								/>
							}
						>
							{willCreateBranch ? (
								<span className={styles.tooltipTarget}>
									<span className={styles.tooltipTargetLabel}>Will create branch:</span>
									<span className={styles.tooltipTargetName}>{draftBranchLabel}</span>
								</span>
							) : commitTarget ? (
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
	);

	if (!isExpanded) {
		return (
			<div {...{ [NO_DRAG_ATTRIBUTE]: "" }} className={classes(styles.startCommitRow, className)}>
				{renderTargetPicker({
					className: classes(
						getButtonClassName({ variant: "outline" }),
						styles.collapsedTargetTrigger,
					),
					iconSize: 14,
				})}

				{/* Amend ignores the message, so its affordance belongs here rather than
				    behind the message composer. Mirrors the hotkeys, which are registered
				    regardless of whether the form is expanded.

				    The accent is spent on the one action this panel exists for, and with
				    nothing staged that is no longer what it is. The button stays live —
				    an empty commit is a real thing to want — but steps back to a quiet
				    outline, and carries the one thing the header does not say: that a
				    commit made now would be empty. */}
				<DropdownButton
					className={styles.startCommitSplit}
					variant={hasWorktreeChanges ? "pop" : "outline"}
					id={startCommitButtonId}
					onClick={() => setIsExpanded(true)}
					disabled={!noOperationPending}
					actionTooltip={hasWorktreeChanges ? undefined : "Makes an empty commit"}
					menuLabel="Commit options"
					menuDisabled={!(canAmend || canCommit)}
					onMenuTrigger={(trigger) => {
						void showNativeMenuFromTrigger(trigger, commitMenuItems());
					}}
				>
					Start commit
					<Kbd hotkey={sidebarHotkeys.composeCommitMessage.hotkey} variant="button" />
				</DropdownButton>
			</div>
		);
	}

	return (
		// oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- Used for persistence, not UI per se.
		<form
			{...{ [NO_DRAG_ATTRIBUTE]: "", [COMMIT_FORM_ATTRIBUTE]: "" }}
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
				disabled={!noOperationPending}
				readOnly={isCommitOrAmendPending || isGenerating}
				placeholder={commitTextareaLabel}
				defaultValue={draftMessage ?? ""}
				className={classes("text-13", "text-body", styles.textarea)}
			/>

			<div className={styles.footer}>
				<div className={styles.footerRow}>
					<div className={styles.footerStart}>
						{renderTargetPicker({
							className: classes(getButtonClassName({ variant: "ghost" }), styles.targetTrigger),
						})}

						<div aria-hidden className={styles.footerSeparator} />
						<Tooltip.Root>
							<Tooltip.Trigger
								aria-label="Generate commit message"
								className={classes(
									getButtonClassName({ variant: "ghost", iconOnly: true }),
									styles.generateButton,
								)}
								onClick={generateCommitMessage}
								render={
									<Button
										focusableWhenDisabled
										type="button"
										disabled={generationButton.disabled}
									/>
								}
							>
								<Icon name={isGenerating ? "spinner" : "ai-text"} />
							</Tooltip.Trigger>
							<Tooltip.Portal>
								<Tooltip.Positioner sideOffset={4}>
									<Tooltip.Popup render={<TooltipPopup />}>
										{generationButton.hint ??
											(isGenerating ? "Generating message…" : "Generate message")}
									</Tooltip.Popup>
								</Tooltip.Positioner>
							</Tooltip.Portal>
						</Tooltip.Root>
					</div>

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
									focusScope("uncommitted-files");
								}}
								render={
									<Button
										focusableWhenDisabled
										disabled={isCommitOrAmendPending || isGenerating}
										type="button"
									/>
								}
							>
								Cancel
							</Tooltip.Trigger>
							<Tooltip.Portal>
								<Tooltip.Positioner sideOffset={4}>
									<Tooltip.Popup render={<TooltipPopup kbd="Escape" />}>Hide form</Tooltip.Popup>
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
			</div>
		</form>
	);
};
