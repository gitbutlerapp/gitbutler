import { absorptionPlanQueryOptions } from "#ui/api/queries.ts";
import {
	getOperations,
	operationLabel,
	useRunOperation,
	type OperationType,
	type OperationsByType,
} from "#ui/operations/operation.ts";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import { Tooltip } from "#ui/components/Tooltip.tsx";
import { Toast, useRender } from "@base-ui/react";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import { FC } from "react";
import styles from "./OperationTooltip.module.css";
import { Operand, operandEquals } from "#ui/operands.ts";
import { useAppDispatch } from "#ui/store.ts";
import { projectActions } from "#ui/projects/state.ts";
import { getTransferOperation, type OutlineMode } from "#ui/outline/mode.ts";
import { Match } from "effect";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useMutation, useQuery } from "@tanstack/react-query";
import { type AbsorptionTarget } from "@gitbutler/but-sdk";
import { errorMessageForToast } from "#ui/errors.ts";
import { operationHotkeys } from "#ui/hotkeys.ts";

const AbsorbControls: FC<{
	projectId: string;
	sourceTarget: AbsorptionTarget;
}> = ({ projectId, sourceTarget }) => {
	const dispatch = useAppDispatch();
	const absorptionPlan = useQuery(absorptionPlanQueryOptions({ projectId, target: sourceTarget }));
	const canAbsorb =
		!absorptionPlan.isPending && !!absorptionPlan.data && absorptionPlan.data.length > 0;
	const toastManager = Toast.useToastManager();
	const absorbMutation = useMutation({
		mutationFn: () => {
			if (!absorptionPlan.data) return Promise.resolve(0);
			return window.lite.absorb({ projectId, absorptionPlan: absorptionPlan.data });
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to absorb",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});

	const confirm = () => {
		dispatch(projectActions.exitMode({ projectId }));

		absorbMutation.mutate();
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
		<>
			<ShortcutButton
				hotkey={operationHotkeys.confirm.hotkey}
				hotkeyOptions={{ meta: operationHotkeys.confirm.meta }}
				onClick={confirm}
				disabled={!canAbsorb}
			>
				Absorb
			</ShortcutButton>
			<ShortcutButton
				hotkey={operationHotkeys.cancel.hotkey}
				hotkeyOptions={{ meta: operationHotkeys.cancel.meta }}
				onClick={cancel}
			>
				Cancel
			</ShortcutButton>
		</>
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

	const setOperationType = (operationType: OperationType) =>
		dispatch(projectActions.updateTransferOperationType({ projectId, operationType }));

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

	const onValueChange = (value: Array<string>) => {
		if (value.length === 0) return;
		const nextOperationType = value[0] as OperationType;

		setOperationType(nextOperationType);
	};

	return (
		<>
			<ToggleGroup
				aria-label="Operation type"
				value={[operationType]}
				onValueChange={onValueChange}
				className={styles.operationTypeToggleGroup}
				orientation="vertical"
			>
				<Toggle
					value={"moveAbove" satisfies OperationType}
					className={styles.operationTypeToggle}
					render={
						<ShortcutButton
							hotkey={operationHotkeys.selectMoveAbove.hotkey}
							hotkeyOptions={{ meta: operationHotkeys.selectMoveAbove.meta }}
						/>
					}
				>
					{operations.moveAbove ? operationLabel(operations.moveAbove) : "Move above"}
				</Toggle>
				<Toggle
					value={"rub" satisfies OperationType}
					className={styles.operationTypeToggle}
					render={
						<ShortcutButton
							hotkey={operationHotkeys.selectRub.hotkey}
							hotkeyOptions={{ meta: operationHotkeys.selectRub.meta }}
						/>
					}
				>
					{operations.rub ? operationLabel(operations.rub) : "Rub"}
				</Toggle>
				<Toggle
					value={"moveBelow" satisfies OperationType}
					className={styles.operationTypeToggle}
					render={
						<ShortcutButton
							hotkey={operationHotkeys.selectMoveBelow.hotkey}
							hotkeyOptions={{ meta: operationHotkeys.selectMoveBelow.meta }}
						/>
					}
				>
					{operations.moveBelow ? operationLabel(operations.moveBelow) : "Move below"}
				</Toggle>
			</ToggleGroup>
			<ShortcutButton
				hotkey={operationHotkeys.confirm.hotkey}
				hotkeyOptions={{ meta: operationHotkeys.confirm.meta }}
				onClick={run}
				disabled={!operation}
			>
				Confirm
			</ShortcutButton>
			<ShortcutButton
				hotkey={operationHotkeys.cancel.hotkey}
				hotkeyOptions={{ meta: operationHotkeys.cancel.meta }}
				onClick={cancel}
			>
				Cancel
			</ShortcutButton>
		</>
	);
};

export const OperationTooltip: FC<
	{
		projectId: string;
		target: Operand;
		outlineMode: OutlineMode;
		isActive: boolean;
	} & useRender.ComponentProps<"div">
> = ({ projectId, target, outlineMode, isActive, render, ...props }) => {
	const tooltip = isActive
		? Match.value(outlineMode).pipe(
				Match.tags({
					Absorb: ({ sourceTarget }) => (
						<AbsorbControls projectId={projectId} sourceTarget={sourceTarget} />
					),
					Transfer: ({ value: mode }) =>
						Match.value(mode).pipe(
							Match.tagsExhaustive({
								Pointer: (mode) => {
									const operation = getTransferOperation({ mode, target });
									if (!operation) return null;

									return <>{operationLabel(operation)}</>;
								},
								Keyboard: (mode) => (
									<>
										{operandEquals(mode.source, target) && <>Select a target</>}
										<TransferOperationControls
											projectId={projectId}
											operations={getOperations(mode.source, target)}
											operationType={mode.operationType}
										/>
									</>
								),
							}),
						),
				}),
				Match.orElse(() => null),
			)
		: null;

	const trigger = useRender({ render, props });

	const isPointerTransfer = Match.value(outlineMode).pipe(
		Match.when({ _tag: "Transfer", value: { _tag: "Pointer" } }, () => true),
		Match.orElse(() => false),
	);

	return (
		<Tooltip
			open={!!tooltip}
			disableHoverablePopup={isPointerTransfer}
			onOpenChange={(_open, eventDetails) => {
				eventDetails.allowPropagation();
			}}
			trigger={trigger}
			content={tooltip ?? undefined}
			positionerProps={{ sideOffset: 8, side: "right" }}
		/>
	);
};
