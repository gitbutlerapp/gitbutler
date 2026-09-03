import { useAbsorb } from "#ui/api/mutations.ts";
import {
	cancelPendingOperation,
	cancelPendingOperationAndRestoreFocus,
	useSelection,
	useActiveList,
} from "#ui/use-cursor.ts";
import { absorptionPlanQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex, type HeadInfoIndex } from "#ui/api/ref-info.ts";
import { getButtonClassName, type ButtonSize, type ButtonVariant } from "#ui/components/Button.tsx";
import { Snackbar } from "#ui/components/Snackbar.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import type { IconName } from "#ui/components/iconNames.ts";
import { Kbd } from "#ui/components/Kbd.tsx";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import {
	Toolbox,
	ToolboxMeta,
	ToolboxMetaHint,
	ToolboxMetaText,
	ToolboxSection,
	ToolboxSeparator,
	ToolboxStack,
} from "#ui/components/Toolbox.tsx";
import { formatForDisplaySorted, operationHotkeys } from "#ui/hotkeys.ts";
import { addressEquals, addressFileParent, type Address } from "#ui/addresses.ts";
import {
	getOperations,
	useExecuteOperation,
	type Placement,
	type OperationsByPlacement,
	type TransferKind,
} from "#ui/operations/operation.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { isCommitFormKeyEvent } from "#ui/routes/project/$id/workspace/commitFormEvent.ts";
import { addressLabel, addressesLabel } from "#ui/routes/project/$id/workspace/addressLabel.ts";
import { useCheckedActions } from "#ui/routes/project/$id/workspace/useCheckedActions.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Button, Toggle, ToggleGroup } from "@base-ui/react";
import { useHotkeys, type UseHotkeyDefinition } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import { useEffect, type FC, type ReactNode } from "react";
import styles from "./OperationControls.module.css";
import {
	type PendingAbsorb,
	getTransferTarget,
	type KeyboardTransfer,
	keyboardTransfer,
} from "#ui/operations/pending-operation.ts";
import type { FocusScope } from "#ui/focus-scopes.ts";
import type { AddressSpace } from "#ui/workspace/address-space.ts";

/** How long a notice stands before it takes itself away. */
const NOTICE_TIMEOUT_MS = 5_000;

const Container: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={styles.container}>
		<ToolboxStack>{children}</ToolboxStack>
	</div>
);

/** The kind of thing an operation is holding, so its toolbox can say so at a glance. */
const iconForAddress = (address: Address | undefined): IconName | undefined => {
	switch (address?._tag) {
		case "Commit":
			return "commit";
		case "File":
			return "file-diff";
		case "Hunk":
			return "diff";
		default:
			return undefined;
	}
};

/**
 * Every act in a toolbox wears the chord that runs it, so the toolbox teaches the keyboard rather
 * than standing in for it.
 */
const ToolboxButton: FC<{
	label: string;
	hotkey?: string;
	variant?: ButtonVariant;
	size?: ButtonSize;
	enabled?: boolean;
	onClick: () => void;
}> = ({ label, hotkey, variant, size, enabled = true, onClick }) => (
	<Button
		className={getButtonClassName({ variant, size })}
		disabled={!enabled}
		focusableWhenDisabled
		onMouseDown={(event) => {
			// Prevent stealing focus from the tree.
			if (!event.defaultPrevented) event.preventDefault();
		}}
		onClick={onClick}
	>
		{label}
		{hotkey !== undefined && <Kbd hotkey={hotkey} variant="button" />}
	</Button>
);

type Confirm = {
	label: string;
	canRun: boolean;
	onRun: () => void;
	extraHotkeys?: Array<Omit<UseHotkeyDefinition, "callback">>;
};

const useControlHotkeys = ({
	onCancel,
	confirm,
}: {
	onCancel: UseHotkeyDefinition["callback"];
	confirm?: Confirm;
}): void => {
	const confirmHotkeys: Array<Omit<UseHotkeyDefinition, "callback">> = [
		...(confirm?.extraHotkeys ?? []),
		{ hotkey: operationHotkeys.confirm.hotkey },
	];

	useHotkeys([
		...(confirm
			? confirmHotkeys.map(
					(hotkey): UseHotkeyDefinition => ({
						hotkey: hotkey.hotkey,
						callback: confirm.onRun,
						options: {
							...hotkey.options,
							conflictBehavior: "allow",
							enabled: confirm.canRun,
						},
					}),
				)
			: []),
		{
			hotkey: operationHotkeys.cancel.hotkey,
			callback: onCancel,
			options: {
				conflictBehavior: "allow",
			},
		},
	]);
};

/**
 * The way out of a toolbox that has nothing pending to abandon: a close affordance rather than a
 * peer of the acts beside it, with its chord stated in the strip above.
 */
const CloseButton: FC<{ onCancel: () => void; size?: ButtonSize }> = ({ onCancel, size }) => (
	<Button
		className={getButtonClassName({ variant: "ghost", iconOnly: true, size })}
		aria-label="Cancel"
		onMouseDown={(event) => {
			// Prevent stealing focus from the tree.
			if (!event.defaultPrevented) event.preventDefault();
		}}
		onClick={onCancel}
	>
		<Icon name="cross" />
	</Button>
);

const CancelButton: FC<{ onCancel: () => void; size?: ButtonSize }> = ({ onCancel, size }) => (
	<ToolboxButton
		label="Cancel"
		hotkey={operationHotkeys.cancel.hotkey}
		size={size}
		onClick={onCancel}
	/>
);

/** What ends an operation, gathered where a dialog would put it. */
const ConfirmSection: FC<{ confirm: Confirm; onCancel: () => void }> = ({ confirm, onCancel }) => (
	<ToolboxSection variant="confirm">
		<ToolboxButton
			label={confirm.label}
			hotkey={operationHotkeys.confirm.hotkey}
			variant="gray"
			size="small"
			enabled={confirm.canRun}
			onClick={confirm.onRun}
		/>
		<CancelButton onCancel={onCancel} size="small" />
	</ToolboxSection>
);

const CheckedAddressOperationControls: FC<{
	checkedAddressCount: number;
	projectId: string;
	appliedAddressSpace: AddressSpace<Address>;
}> = ({ checkedAddressCount, projectId, appliedAddressSpace }) => {
	const dispatch = useAppDispatch();

	// A primitive, so a check that leaves the context alone doesn't re-render the bar.
	const checkedContext = useAppSelector((state) =>
		projectSlice.selectors.selectCheckedAddressesContext(state, projectId),
	);
	const actions = useCheckedActions({ projectId, appliedAddressSpace });

	const cancel = () => {
		dispatch(projectSlice.actions.clearCheckedAddresses({ projectId }));
	};
	useControlHotkeys({
		// The commit form closes on the same press, and the files checked for it to commit are the
		// one thing that has to outlive it.
		onCancel: (event) => {
			if (isCommitFormKeyEvent(event)) return;
			cancel();
		},
	});

	if (checkedContext === null) return;

	const { noun, icon } = Match.value(checkedContext).pipe(
		Match.withReturnType<{ noun: string; icon: IconName }>(),
		Match.when("Commit", () => ({ noun: "commit", icon: "commit" as const })),
		Match.when("File", () => ({ noun: "file", icon: "file-diff" as const })),
		Match.when("Hunk", () => ({ noun: "line", icon: "diff" as const })),
		Match.exhaustive,
	);

	return (
		<Toolbox>
			<ToolboxMeta icon={icon}>
				<span>
					{new Intl.NumberFormat().format(checkedAddressCount)} {noun}
					{new Intl.PluralRules().select(checkedAddressCount) !== "one" && "s"} selected
				</span>
				<ToolboxMetaHint>
					{formatForDisplaySorted(operationHotkeys.cancel.hotkey)} to close
				</ToolboxMetaHint>
			</ToolboxMeta>
			<ToolboxSection>
				{actions.map((action) => (
					<ToolboxButton
						key={action.label}
						label={action.label}
						hotkey={action.hotkey}
						variant={action.variant}
						enabled={action.enabled}
						onClick={action.run}
					/>
				))}
				{actions.length > 0 && <ToolboxSeparator />}
				<CloseButton onCancel={cancel} />
			</ToolboxSection>
		</Toolbox>
	);
};

const AbsorbOperationControls: FC<{
	headInfoIndex: HeadInfoIndex;
	projectId: string;
	pending: PendingAbsorb;
}> = ({ headInfoIndex, projectId, pending }) => {
	const dispatch = useAppDispatch();
	const {
		data: absorptionPlan,
		isError: isAbsorptionPlanError,
		isPending: isAbsorptionPlanPending,
	} = useQuery(absorptionPlanQueryOptions({ projectId, target: pending.sourceTarget }));
	const canAbsorb = !isAbsorptionPlanPending && !!absorptionPlan && absorptionPlan.length > 0;
	const { mutate: absorb } = useAbsorb({ projectId });

	const run = () => {
		dispatch(projectSlice.actions.clearPendingOperation({ projectId }));

		absorb(absorptionPlan);
	};

	const cancel = () => {
		cancelPendingOperation();
	};

	const confirm: Confirm = { label: "Absorb", canRun: canAbsorb, onRun: run };
	useControlHotkeys({ onCancel: cancel, confirm });

	const sourcesLabel = addressesLabel({ headInfoIndex, addresses: pending.sources });

	// The plan answers where these changes belong. With no answer — because nothing owns them, or
	// because working it out failed — there is nothing left to aim, and an operation that cannot be
	// aimed must not hold the workspace open: it stands down and leaves a notice saying why.
	const refusal = isAbsorptionPlanPending
		? null
		: isAbsorptionPlanError
			? `Couldn’t work out where to absorb ${sourcesLabel}`
			: canAbsorb
				? null
				: `Nothing to absorb ${sourcesLabel} into`;

	useEffect(() => {
		if (refusal === null) return;

		dispatch(projectSlice.actions.refusePendingOperation({ projectId, notice: refusal }));
	}, [dispatch, projectId, refusal]);

	if (refusal !== null) return null;

	return (
		<Container>
			<Toolbox>
				<ToolboxMeta icon={isAbsorptionPlanPending ? "spinner" : "absorb"}>
					{isAbsorptionPlanPending ? (
						<span>Loading absorb plan</span>
					) : (
						<>
							<strong>Absorb</strong>
							<ToolboxMetaText>{sourcesLabel}</ToolboxMetaText>
							<strong>into {absorptionPlan?.length ?? 0} commits</strong>
						</>
					)}
				</ToolboxMeta>
				<ConfirmSection confirm={confirm} onCancel={cancel} />
			</Toolbox>
		</Container>
	);
};

const TransferTypeToggleGroup: FC<{
	projectId: string;
	operations: OperationsByPlacement;
	placement: Placement;
}> = ({ projectId, operations, placement }) => {
	const dispatch = useAppDispatch();

	const setPlacement = (placement: Placement) =>
		dispatch(projectSlice.actions.updateTransferPlacement({ projectId, placement }));

	useHotkeys([
		{
			hotkey: operationHotkeys.selectAbove.hotkey,
			callback: () => setPlacement("above"),
			options: {
				conflictBehavior: "allow",
				enabled: !!operations.above,
			},
		},
		{
			hotkey: operationHotkeys.selectInto.hotkey,
			callback: () => setPlacement("into"),
			options: {
				conflictBehavior: "allow",
				enabled: !!operations.into,
			},
		},
		{
			hotkey: operationHotkeys.selectBelow.hotkey,
			callback: () => setPlacement("below"),
			options: {
				conflictBehavior: "allow",
				enabled: !!operations.below,
			},
		},
	]);

	const onValueChange = (value: Array<string>) => {
		if (value.length === 0) return;
		const nextPlacement = value[0] as Placement;

		setPlacement(nextPlacement);
	};

	return (
		<ToggleGroup
			aria-label="Placement"
			value={[placement]}
			onValueChange={onValueChange}
			render={<ToggleGroupStyles />}
			onMouseDown={(event) => {
				// Prevent stealing focus from the tree.
				if (!event.defaultPrevented) event.preventDefault();
			}}
		>
			<Toggle
				render={<ToggleStyles />}
				value={"above" satisfies Placement}
				disabled={!operations.above}
			>
				Above <Kbd hotkey={operationHotkeys.selectAbove.hotkey} variant="button" />
			</Toggle>

			<Toggle
				render={<ToggleStyles />}
				value={"into" satisfies Placement}
				disabled={!operations.into}
			>
				Into <Kbd hotkey={operationHotkeys.selectInto.hotkey} variant="button" />
			</Toggle>

			<Toggle
				render={<ToggleStyles />}
				value={"below" satisfies Placement}
				disabled={!operations.below}
			>
				Below <Kbd hotkey={operationHotkeys.selectBelow.hotkey} variant="button" />
			</Toggle>
		</ToggleGroup>
	);
};

const TransferKindToggleGroup: FC<{
	kind: TransferKind;
	projectId: string;
}> = ({ kind, projectId }) => {
	const dispatch = useAppDispatch();

	const setKind = (kind: TransferKind) =>
		dispatch(projectSlice.actions.updateTransferKind({ projectId, kind }));

	useHotkeys([
		{
			hotkey: operationHotkeys.selectMove.hotkey,
			callback: () => setKind("move"),
			options: { conflictBehavior: "allow" },
		},
		{
			hotkey: operationHotkeys.selectCopy.hotkey,
			callback: () => setKind("copy"),
			options: { conflictBehavior: "allow" },
		},
	]);

	const onValueChange = (value: Array<string>) => {
		if (value.length === 0) return;
		setKind(value[0] as TransferKind);
	};

	return (
		<ToggleGroup
			aria-label="Transfer kind"
			value={[kind]}
			onValueChange={onValueChange}
			render={<ToggleGroupStyles />}
			onMouseDown={(evt) => {
				// Prevent stealing focus from the tree.
				if (!evt.defaultPrevented) evt.preventDefault();
			}}
		>
			<Toggle render={<ToggleStyles />} value={"move" satisfies TransferKind}>
				Move <Kbd hotkey={operationHotkeys.selectMove.hotkey} variant="button" />
			</Toggle>

			<Toggle render={<ToggleStyles />} value={"copy" satisfies TransferKind}>
				Copy <Kbd hotkey={operationHotkeys.selectCopy.hotkey} variant="button" />
			</Toggle>
		</ToggleGroup>
	);
};

/**
 * Why a target refuses everything it was offered. A transfer aims by moving the cursor, so a target
 * that resolves to nothing is a step in aiming rather than a failure: the strip says what it cannot
 * do and leaves the ways out — a different target, a different kind — where they already were.
 */
const transferRefusal = ({
	headInfoIndex,
	sources,
	target,
	kind,
}: {
	headInfoIndex: HeadInfoIndex;
	sources: Array<Address>;
	target: Address;
	kind: TransferKind;
}): ReactNode => {
	const sourcesLabel = (
		<ToolboxMetaText>{addressesLabel({ headInfoIndex, addresses: sources })}</ToolboxMetaText>
	);

	// Uncommitted changes are one bucket with no order and no lanes, so a change already in it has
	// nowhere within it to go.
	if (
		target._tag === "UncommittedChanges" &&
		sources.length > 0 &&
		sources.every((source) => addressFileParent(source)?._tag === "UncommittedChanges")
	) {
		return (
			<>
				<strong>Already uncommitted:</strong>
				{sourcesLabel}
			</>
		);
	}

	const verb = kind === "copy" ? "copy" : "move";

	if (sources.some((source) => addressEquals(source, target))) {
		return (
			<>
				<strong>Can’t {verb}</strong>
				{sourcesLabel}
				<strong>onto {sources.length === 1 ? "itself" : "themselves"}</strong>
			</>
		);
	}

	return (
		<>
			<strong>Can’t {verb}</strong>
			{sourcesLabel}
			<strong>to</strong>
			<ToolboxMetaText>{addressLabel({ headInfoIndex, address: target })}</ToolboxMetaText>
		</>
	);
};

const TransferKeyboardOperationControls: FC<{
	headInfoIndex: HeadInfoIndex;
	projectId: string;
	transfer: KeyboardTransfer;
	appliedAddressSpace: AddressSpace<Address>;
	onFocusRestore: (scope: FocusScope) => void;
}> = ({ headInfoIndex, projectId, transfer, appliedAddressSpace, onFocusRestore }) => {
	const activeList = useActiveList();
	const selection = useSelection("applied", appliedAddressSpace);

	const dispatch = useAppDispatch();
	const { mutate: executeOperation } = useExecuteOperation(projectId);

	const target = getTransferTarget(keyboardTransfer(transfer), selection, activeList);

	const operations = target ? getOperations(transfer.sources, target, transfer.kind) : null;
	const operation = operations?.[transfer.placement];
	// Three disabled placements and a confirm that cannot be taken say only that something is
	// wrong. When no placement resolves, the toolbox states the reason instead.
	const refusal =
		target && operations && !operations.into && !operations.above && !operations.below
			? transferRefusal({ headInfoIndex, sources: transfer.sources, target, kind: transfer.kind })
			: null;
	const canCopy =
		transfer.sources.length > 0 && transfer.sources.every((source) => source._tag === "Commit");

	const run = () => {
		dispatch(projectSlice.actions.clearPendingOperation({ projectId }));

		if (!operation) return;

		executeOperation(operation.operation);
	};

	const cancel = () => {
		cancelPendingOperationAndRestoreFocus(onFocusRestore);
	};

	const confirm: Confirm = {
		// The placement toggles say where; the operation they resolve to says what, so the
		// confirm button is where it belongs.
		label: operation?.label ?? "Confirm",
		canRun: !!operation,
		onRun: run,
		extraHotkeys: [{ hotkey: operationHotkeys.confirmTransfer.hotkey }],
	};
	useControlHotkeys({ onCancel: cancel, confirm });

	if (!target || !operations) return null;

	return (
		<Container>
			{/* Only commits can be copied, so for any other source the addon offers a choice of one:
			    the kind is already `move` and cannot become anything else. */}
			{canCopy && (
				<Toolbox>
					<ToolboxSection variant="stretch">
						<TransferKindToggleGroup kind={transfer.kind} projectId={projectId} />
					</ToolboxSection>
				</Toolbox>
			)}
			<Toolbox>
				{refusal !== null ? (
					<>
						<ToolboxMeta icon="warning">{refusal}</ToolboxMeta>
						<ToolboxSection variant="confirm">
							<CancelButton onCancel={cancel} size="small" />
						</ToolboxSection>
					</>
				) : (
					<>
						<ToolboxMeta icon={iconForAddress(transfer.sources[0])}>
							<strong>{transfer.kind === "copy" ? "Copy" : "Move"}</strong>
							<ToolboxMetaText>
								{addressesLabel({ headInfoIndex, addresses: transfer.sources })}
							</ToolboxMetaText>
							<strong>{transfer.placement}</strong>
							<ToolboxMetaText>{addressLabel({ headInfoIndex, address: target })}</ToolboxMetaText>
						</ToolboxMeta>
						<ToolboxSection variant="stretch">
							<TransferTypeToggleGroup
								projectId={projectId}
								operations={operations}
								placement={transfer.placement}
							/>
						</ToolboxSection>
						<ConfirmSection confirm={confirm} onCancel={cancel} />
					</>
				)}
			</Toolbox>
		</Container>
	);
};

export const OperationControls: FC<{
	projectId: string;
	appliedAddressSpace: AddressSpace<Address>;
	onFocusRestore: (scope: FocusScope) => void;
}> = ({ projectId, appliedAddressSpace, onFocusRestore }) => {
	const pendingOperation = useAppSelector((state) =>
		projectSlice.selectors.selectPendingOperation(state, projectId),
	);
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const checkedAddressCount = useAppSelector((state) =>
		projectSlice.selectors.selectCheckedAddressCount(state, projectId),
	);
	const notice = useAppSelector((state) => projectSlice.selectors.selectNotice(state, projectId));
	const dispatch = useAppDispatch();
	const clearNotice = () => dispatch(projectSlice.actions.clearNotice({ projectId }));

	// A notice is the end of something, not a thing to attend to: it carries no way out of its own
	// and leaves on a click or on its own, so the only close button on screen belongs to whatever
	// the workspace still has in hand.
	useEffect(() => {
		if (notice === null) return;

		const timer = setTimeout(
			() => dispatch(projectSlice.actions.clearNotice({ projectId })),
			NOTICE_TIMEOUT_MS,
		);
		return () => clearTimeout(timer);
	}, [dispatch, notice, projectId]);

	return Match.value(pendingOperation).pipe(
		Match.tagsExhaustive({
			None: () =>
				(notice !== null || checkedAddressCount > 0) && (
					<div className={styles.container}>
						<ToolboxStack>
							{notice !== null && (
								<Snackbar variant="danger" className={styles.notice} onClick={clearNotice}>
									{notice}
								</Snackbar>
							)}
							{checkedAddressCount > 0 && (
								<CheckedAddressOperationControls
									checkedAddressCount={checkedAddressCount}
									projectId={projectId}
									appliedAddressSpace={appliedAddressSpace}
								/>
							)}
						</ToolboxStack>
					</div>
				),
			Absorb: (pending) =>
				headInfoIndex && (
					<AbsorbOperationControls
						headInfoIndex={headInfoIndex}
						projectId={projectId}
						pending={pending}
					/>
				),
			Transfer: ({ value: transfer }) =>
				Match.value(transfer).pipe(
					Match.tags({
						Keyboard: (transfer) =>
							headInfoIndex && (
								<TransferKeyboardOperationControls
									headInfoIndex={headInfoIndex}
									projectId={projectId}
									transfer={transfer}
									appliedAddressSpace={appliedAddressSpace}
									onFocusRestore={onFocusRestore}
								/>
							),
					}),
					Match.orElse(() => null),
				),
			InlineEdit: () => null,
		}),
	);
};
