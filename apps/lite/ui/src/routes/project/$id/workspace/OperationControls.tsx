import { useAbsorb } from "#ui/api/mutations.ts";
import { cancelPendingOperation, useSelection, useActiveList } from "#ui/use-cursor.ts";
import { absorptionPlanQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex, type HeadInfoIndex } from "#ui/api/ref-info.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { Kbd } from "#ui/components/Kbd.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { operationHotkeys } from "#ui/hotkeys.ts";
import type { Address } from "#ui/addresses.ts";
import {
	getOperations,
	useExecuteOperation,
	type Placement,
	type OperationsByPlacement,
	type TransferKind,
} from "#ui/operations/operation.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { addressLabel, addressesLabel } from "#ui/routes/project/$id/workspace/addressLabel.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Button, Toggle, ToggleGroup, Tooltip } from "@base-ui/react";
import { useHotkeys, type UseHotkeyDefinition } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import type { FC, ReactNode } from "react";
import styles from "./OperationControls.module.css";
import {
	type PendingAbsorb,
	getTransferTarget,
	type KeyboardTransfer,
	keyboardTransfer,
} from "#ui/operations/pending-operation.ts";
import type { AddressSpace } from "#ui/workspace/address-space.ts";

const Container: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={classes("text-14", styles.container)}>{children}</div>
);

const ControlsRow: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={styles.controlsRow}>{children}</div>
);

const Label: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={classes(styles.label, "text-bold", "text-13")}>{children}</div>
);

const Separator: FC = () => <div className={styles.separator} />;

const Controls: FC<{
	onCancel: () => void;
	confirm?: {
		canRun: boolean;
		onRun: () => void;
		extraHotkeys?: Array<Omit<UseHotkeyDefinition, "callback">>;
	};
}> = ({ onCancel, confirm }) => {
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

	return (
		<div className={styles.controls}>
			{confirm && (
				<Tooltip.Root>
					<Tooltip.Trigger
						className={getButtonClassName({ variant: "gray" })}
						onMouseDown={(event) => {
							// Prevent stealing focus from the tree.
							if (!event.defaultPrevented) event.preventDefault();
						}}
						onClick={confirm.onRun}
						// We pass `disabled` here because we want to disable the button, not
						// the tooltip. Other props should be passed above.
						render={<Button focusableWhenDisabled disabled={!confirm.canRun} />}
					>
						Confirm
					</Tooltip.Trigger>
					<Tooltip.Portal>
						<Tooltip.Positioner sideOffset={4}>
							<Tooltip.Popup render={<TooltipPopup kbd={operationHotkeys.confirm.hotkey} />}>
								Confirm
							</Tooltip.Popup>
						</Tooltip.Positioner>
					</Tooltip.Portal>
				</Tooltip.Root>
			)}

			<Tooltip.Root>
				<Tooltip.Trigger
					className={getButtonClassName({})}
					onMouseDown={(event) => {
						// Prevent stealing focus from the tree.
						if (!event.defaultPrevented) event.preventDefault();
					}}
					onClick={onCancel}
				>
					Cancel
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup render={<TooltipPopup kbd={operationHotkeys.cancel.hotkey} />}>
							Cancel
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
		</div>
	);
};

const CheckedAddressOperationControls: FC<{ checkedAddressCount: number; projectId: string }> = ({
	checkedAddressCount,
	projectId,
}) => {
	const dispatch = useAppDispatch();

	const checkedType = useAppSelector((state): string | null => {
		switch (projectSlice.selectors.selectCheckedAddressesContext(state, projectId)) {
			case "Commit":
				return "commit";
			case "File":
				return "file";
			case "Hunk":
				return "line";
			case null:
				return null;
		}
	});
	if (checkedType === null) return;

	const cancel = () => {
		dispatch(projectSlice.actions.clearCheckedAddresses({ projectId }));
	};

	return (
		<Container>
			<ControlsRow>
				<Label>
					{new Intl.NumberFormat().format(checkedAddressCount)} {checkedType}
					{new Intl.PluralRules().select(checkedAddressCount) !== "one" && "s"} selected
				</Label>
				<Controls onCancel={cancel} />
			</ControlsRow>
		</Container>
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

	return (
		<Container>
			<ControlsRow>
				{isAbsorptionPlanPending ? (
					<Icon name="spinner" aria-label="Loading absorb plan" />
				) : isAbsorptionPlanError ? (
					<Label>Failed to load absorb plan</Label>
				) : (
					<Label>
						Absorb {addressesLabel({ headInfoIndex, addresses: pending.sources })} into{" "}
						{absorptionPlan.length} commits
					</Label>
				)}
				<Controls onCancel={cancel} confirm={{ canRun: canAbsorb, onRun: run }} />
			</ControlsRow>
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
			className={styles.toggleGroupRow}
			onMouseDown={(event) => {
				// Prevent stealing focus from the tree.
				if (!event.defaultPrevented) event.preventDefault();
			}}
		>
			<Toggle
				className={styles.toggleGroupRowToggle}
				value={"above" satisfies Placement}
				disabled={!operations.above}
			>
				{operations.above && (
					<div className={classes("text-12", styles.operationLabel)}>{operations.above.label}</div>
				)}
				<div className="text-semibold">
					Above <Kbd hotkey={operationHotkeys.selectAbove.hotkey} />
				</div>
			</Toggle>

			<Toggle
				className={styles.toggleGroupRowToggle}
				value={"into" satisfies Placement}
				disabled={!operations.into}
			>
				{operations.into && (
					<div className={classes("text-12", styles.operationLabel)}>{operations.into.label}</div>
				)}
				<div className="text-semibold">
					Into <Kbd hotkey={operationHotkeys.selectInto.hotkey} />
				</div>
			</Toggle>

			<Toggle
				className={styles.toggleGroupRowToggle}
				value={"below" satisfies Placement}
				disabled={!operations.below}
			>
				{operations.below && (
					<div className={classes("text-12", styles.operationLabel)}>{operations.below.label}</div>
				)}
				<div className="text-semibold">
					Below <Kbd hotkey={operationHotkeys.selectBelow.hotkey} />
				</div>
			</Toggle>
		</ToggleGroup>
	);
};

const TransferKindToggleGroup: FC<{
	canCopy: boolean;
	kind: TransferKind;
	projectId: string;
}> = ({ canCopy, kind, projectId }) => {
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
			options: { conflictBehavior: "allow", enabled: canCopy },
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
			className={styles.toggleGroupRow}
			onMouseDown={(evt) => {
				// Prevent stealing focus from the tree.
				if (!evt.defaultPrevented) evt.preventDefault();
			}}
		>
			<Toggle className={styles.toggleGroupRowToggle} value={"move" satisfies TransferKind}>
				<div className="text-semibold">
					Move <Kbd hotkey={operationHotkeys.selectMove.hotkey} />
				</div>
			</Toggle>

			<Toggle
				className={styles.toggleGroupRowToggle}
				value={"copy" satisfies TransferKind}
				disabled={!canCopy}
			>
				<div className="text-semibold">
					Copy <Kbd hotkey={operationHotkeys.selectCopy.hotkey} />
				</div>
			</Toggle>
		</ToggleGroup>
	);
};

const TransferKeyboardOperationControls: FC<{
	headInfoIndex: HeadInfoIndex;
	projectId: string;
	transfer: KeyboardTransfer;
	appliedAddressSpace: AddressSpace<Address>;
}> = ({ headInfoIndex, projectId, transfer, appliedAddressSpace }) => {
	const activeList = useActiveList();
	const selection = useSelection("applied", appliedAddressSpace);

	const dispatch = useAppDispatch();
	const { mutate: executeOperation } = useExecuteOperation(projectId);

	const target = getTransferTarget(keyboardTransfer(transfer), selection, activeList);
	if (!target) return null;

	const operations = getOperations(transfer.sources, target, transfer.kind);
	const operation = operations[transfer.placement];
	const canCopy =
		transfer.sources.length > 0 && transfer.sources.every((source) => source._tag === "Commit");

	const run = () => {
		dispatch(projectSlice.actions.clearPendingOperation({ projectId }));

		if (!operation) return;

		executeOperation(operation.operation);
	};

	const cancel = () => {
		cancelPendingOperation();
	};

	return (
		<Container>
			<TransferKindToggleGroup canCopy={canCopy} kind={transfer.kind} projectId={projectId} />
			<TransferTypeToggleGroup
				projectId={projectId}
				operations={operations}
				placement={transfer.placement}
			/>
			<Separator />
			<ControlsRow>
				<Label>
					<div>Source: {addressesLabel({ headInfoIndex, addresses: transfer.sources })}</div>
					<div>Target: {addressLabel({ headInfoIndex, address: target })}</div>
				</Label>
				<Controls
					onCancel={cancel}
					confirm={{
						canRun: !!operation,
						onRun: run,
						extraHotkeys: [
							{
								hotkey: operationHotkeys.confirmTransfer.hotkey,
							},
						],
					}}
				/>
			</ControlsRow>
		</Container>
	);
};

export const OperationControls: FC<{
	projectId: string;
	appliedAddressSpace: AddressSpace<Address>;
}> = ({ projectId, appliedAddressSpace }) => {
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

	return Match.value(pendingOperation).pipe(
		Match.tagsExhaustive({
			None: () =>
				checkedAddressCount > 0 && (
					<CheckedAddressOperationControls
						checkedAddressCount={checkedAddressCount}
						projectId={projectId}
					/>
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
								/>
							),
					}),
					Match.orElse(() => null),
				),
			InlineEdit: () => null,
		}),
	);
};
