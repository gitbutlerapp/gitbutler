import { useAbsorb } from "#ui/api/mutations.ts";
import { absorptionPlanQueryOptions } from "#ui/api/queries.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { operationHotkeys } from "#ui/hotkeys.ts";
import { Operand, operandEquals } from "#ui/operands.ts";
import {
	getOperations,
	operationLabel,
	useRunOperation,
	type OperationType,
	type OperationsByType,
} from "#ui/operations/operation.ts";
import { projectActions, selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Button, Tooltip, useRender } from "@base-ui/react";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import { type AbsorptionTarget } from "@gitbutler/but-sdk";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import { FC } from "react";
import styles from "./OutlineModeTooltip.module.css";

const AbsorbControls: FC<{
	projectId: string;
	sourceTarget: AbsorptionTarget;
}> = ({ projectId, sourceTarget }) => {
	const dispatch = useAppDispatch();
	const absorptionPlan = useQuery(absorptionPlanQueryOptions({ projectId, target: sourceTarget }));
	const canAbsorb =
		!absorptionPlan.isPending && !!absorptionPlan.data && absorptionPlan.data.length > 0;
	const absorbMutation = useAbsorb({ projectId });

	const confirm = () => {
		dispatch(projectActions.exitMode({ projectId }));

		absorbMutation.mutate(absorptionPlan.data);
	};

	const cancel = () => dispatch(projectActions.cancelMode({ projectId }));

	useHotkeys([
		{
			hotkey: operationHotkeys.confirm.hotkey,
			callback: confirm,
			options: {
				conflictBehavior: "allow",
				enabled: canAbsorb,
				meta: operationHotkeys.confirm.meta,
			},
		},
		{
			hotkey: operationHotkeys.cancel.hotkey,
			callback: cancel,
			options: {
				conflictBehavior: "allow",
				meta: operationHotkeys.cancel.meta,
			},
		},
	]);

	return (
		<div className={styles.controls}>
			<Tooltip.Root>
				<Tooltip.Trigger
					className={getButtonClassName({})}
					onClick={confirm}
					// We pass `disabled` here because we want to disable the button, not
					// the tooltip. Other props should be passed above.
					render={<Button focusableWhenDisabled disabled={!canAbsorb} />}
				>
					Absorb
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup render={<TooltipPopup kbd={operationHotkeys.confirm.hotkey} />}>
							{operationHotkeys.confirm.meta.name}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
			<Tooltip.Root>
				<Tooltip.Trigger className={getButtonClassName({})} onClick={cancel}>
					Cancel
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup render={<TooltipPopup kbd={operationHotkeys.cancel.hotkey} />}>
							{operationHotkeys.cancel.meta.name}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
		</div>
	);
};

const TransferTypeToggleGroup: FC<{
	projectId: string;
	operations: OperationsByType;
	operationType: OperationType;
}> = ({ projectId, operations, operationType }) => {
	const dispatch = useAppDispatch();

	const setOperationType = (operationType: OperationType) =>
		dispatch(projectActions.updateTransferOperationType({ projectId, operationType }));

	useHotkeys([
		{
			hotkey: operationHotkeys.selectMoveAbove.hotkey,
			callback: () => setOperationType("moveAbove"),
			options: {
				conflictBehavior: "allow",
				meta: operationHotkeys.selectMoveAbove.meta,
			},
		},
		{
			hotkey: operationHotkeys.selectRub.hotkey,
			callback: () => setOperationType("rub"),
			options: {
				conflictBehavior: "allow",
				meta: operationHotkeys.selectRub.meta,
			},
		},
		{
			hotkey: operationHotkeys.selectMoveBelow.hotkey,
			callback: () => setOperationType("moveBelow"),
			options: {
				conflictBehavior: "allow",
				meta: operationHotkeys.selectMoveBelow.meta,
			},
		},
	]);

	const onValueChange = (value: Array<string>) => {
		if (value.length === 0) return;
		const nextOperationType = value[0] as OperationType;

		setOperationType(nextOperationType);
	};

	return (
		<ToggleGroup
			render={<ToggleGroupStyles />}
			aria-label="Operation type"
			value={[operationType]}
			onValueChange={onValueChange}
			orientation="vertical"
		>
			<Tooltip.Root>
				<Toggle
					value={"moveAbove" satisfies OperationType}
					render={<Tooltip.Trigger render={<ToggleStyles />} />}
				>
					{operations.moveAbove ? operationLabel(operations.moveAbove) : "Move above"}
				</Toggle>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4} side="right">
						<Tooltip.Popup render={<TooltipPopup kbd={operationHotkeys.selectMoveAbove.hotkey} />}>
							{operationHotkeys.selectMoveAbove.meta.name}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>

			<Tooltip.Root>
				<Toggle
					value={"rub" satisfies OperationType}
					render={<Tooltip.Trigger render={<ToggleStyles />} />}
				>
					{operations.rub ? operationLabel(operations.rub) : "Rub"}
				</Toggle>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4} side="right">
						<Tooltip.Popup render={<TooltipPopup kbd={operationHotkeys.selectRub.hotkey} />}>
							{operationHotkeys.selectRub.meta.name}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>

			<Tooltip.Root>
				<Toggle
					value={"moveBelow" satisfies OperationType}
					render={<Tooltip.Trigger render={<ToggleStyles />} />}
				>
					{operations.moveBelow ? operationLabel(operations.moveBelow) : "Move below"}
				</Toggle>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4} side="right">
						<Tooltip.Popup render={<TooltipPopup kbd={operationHotkeys.selectMoveBelow.hotkey} />}>
							{operationHotkeys.selectMoveBelow.meta.name}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
		</ToggleGroup>
	);
};

const TransferOperationControls: FC<{
	projectId: string;
	operations: OperationsByType;
	operationType: OperationType;
}> = ({ projectId, operations, operationType }) => {
	const dispatch = useAppDispatch();
	const { mutate: runOperation } = useRunOperation();
	const operation = operations[operationType];

	const run = () => {
		dispatch(projectActions.exitMode({ projectId }));

		if (!operation) return;

		runOperation(operation);
	};

	const cancel = () => dispatch(projectActions.cancelMode({ projectId }));

	useHotkeys([
		{
			hotkey: operationHotkeys.confirmTransfer.hotkey,
			callback: run,
			options: {
				conflictBehavior: "allow",
				enabled: !!operation,
				ignoreInputs: true,
				meta: operationHotkeys.confirmTransfer.meta,
			},
		},
		{
			hotkey: operationHotkeys.confirm.hotkey,
			callback: run,
			options: {
				conflictBehavior: "allow",
				enabled: !!operation,
				meta: operationHotkeys.confirm.meta,
			},
		},
		{
			hotkey: operationHotkeys.cancel.hotkey,
			callback: cancel,
			options: {
				conflictBehavior: "allow",
				meta: operationHotkeys.cancel.meta,
			},
		},
	]);

	return (
		<div className={styles.controls}>
			<Tooltip.Root>
				<Tooltip.Trigger
					className={getButtonClassName({})}
					onClick={run}
					// We pass `disabled` here because we want to disable the button, not
					// the tooltip. Other props should be passed above.
					render={<Button focusableWhenDisabled disabled={!operation} />}
				>
					Confirm
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup render={<TooltipPopup kbd={operationHotkeys.confirm.hotkey} />}>
							{operationHotkeys.confirm.meta.name}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>

			<Tooltip.Root>
				<Tooltip.Trigger className={getButtonClassName({})} onClick={cancel}>
					Cancel
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup render={<TooltipPopup kbd={operationHotkeys.cancel.hotkey} />}>
							{operationHotkeys.cancel.meta.name}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
		</div>
	);
};

export const OutlineModeTooltip: FC<
	{
		projectId: string;
		target: Operand;
		isActive: boolean;
	} & useRender.ComponentProps<"div">
> = ({ projectId, target, isActive, render, ...props }) => {
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const tooltip = isActive
		? Match.value(outlineMode).pipe(
				Match.tags({
					Absorb: ({ sourceTarget }) => (
						<AbsorbControls projectId={projectId} sourceTarget={sourceTarget} />
					),
					Transfer: ({ value: mode }) =>
						Match.value(mode).pipe(
							Match.tags({
								Keyboard: (mode) => (
									<div className={styles.transferOperation}>
										{operandEquals(mode.source, target) && <>Select a target</>}
										<TransferTypeToggleGroup
											projectId={projectId}
											operations={getOperations(mode.source, target)}
											operationType={mode.operationType}
										/>
										<TransferOperationControls
											projectId={projectId}
											operations={getOperations(mode.source, target)}
											operationType={mode.operationType}
										/>
									</div>
								),
							}),
							Match.orElse(() => null),
						),
				}),
				Match.orElse(() => null),
			)
		: null;

	const trigger = useRender({ render, props });

	return (
		<Tooltip.Root open={!!tooltip}>
			<Tooltip.Trigger render={trigger} />
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8} side="right">
					<Tooltip.Popup render={<TooltipPopup>{tooltip}</TooltipPopup>} />
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
