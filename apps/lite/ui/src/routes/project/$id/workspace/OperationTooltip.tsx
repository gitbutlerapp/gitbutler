import { absorptionPlanQueryOptions } from "#ui/api/queries.ts";
import {
	getOperations,
	operationLabel,
	useRunOperation,
	type OperationType,
	type OperationsByType,
} from "#ui/operations/operation.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Toast, Tooltip, useRender } from "@base-ui/react";
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
import { type AbsorptionTarget, type CommitAbsorption } from "@gitbutler/but-sdk";
import { errorMessageForToast } from "#ui/errors.ts";
import { operationHotkeys } from "#ui/hotkeys.ts";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";

const useAbsorb = ({ projectId }: { projectId: string }) => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (absorptionPlan: Array<CommitAbsorption> | undefined) => {
			if (!absorptionPlan) return Promise.resolve(0);
			return window.lite.absorb({ projectId, absorptionPlan });
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
};

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
		<>
			<Tooltip.Root disabled={!canAbsorb}>
				<Tooltip.Trigger
					className={getButtonClassName({})}
					onClick={confirm}
					// This is needed to ensure the `disabled` attribute is passed to the
					// button element. Other props should be passed above.
					render={<button type="button" disabled={!canAbsorb} />}
				>
					Absorb
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup
							render={
								<TooltipPopup
									content={operationHotkeys.confirm.meta.name}
									kbd={operationHotkeys.confirm.hotkey}
								/>
							}
						/>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
			<Tooltip.Root>
				<Tooltip.Trigger className={getButtonClassName({})} onClick={cancel}>
					Cancel
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup
							render={
								<TooltipPopup
									content={operationHotkeys.cancel.meta.name}
									kbd={operationHotkeys.cancel.hotkey}
								/>
							}
						/>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
		</>
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
						<Tooltip.Popup
							render={
								<TooltipPopup
									content={operationHotkeys.selectMoveAbove.meta.name}
									kbd={operationHotkeys.selectMoveAbove.hotkey}
								/>
							}
						/>
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
						<Tooltip.Popup
							render={
								<TooltipPopup
									content={operationHotkeys.selectRub.meta.name}
									kbd={operationHotkeys.selectRub.hotkey}
								/>
							}
						/>
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
						<Tooltip.Popup
							render={
								<TooltipPopup
									content={operationHotkeys.selectMoveBelow.meta.name}
									kbd={operationHotkeys.selectMoveBelow.hotkey}
								/>
							}
						/>
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
		<div className={styles.keyboardTooltipControls}>
			<Tooltip.Root disabled={!operation}>
				<Tooltip.Trigger
					className={getButtonClassName({})}
					onClick={run}
					// This is needed to ensure the `disabled` attribute is passed
					// to the button element. Other props should be passed above.
					render={<button type="button" disabled={!operation} />}
				>
					Confirm
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup
							render={
								<TooltipPopup
									content={operationHotkeys.confirm.meta.name}
									kbd={operationHotkeys.confirm.hotkey}
								/>
							}
						/>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>

			<Tooltip.Root>
				<Tooltip.Trigger className={getButtonClassName({})} onClick={cancel}>
					Cancel
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup
							render={
								<TooltipPopup
									content={operationHotkeys.cancel.meta.name}
									kbd={operationHotkeys.cancel.hotkey}
								/>
							}
						/>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
		</div>
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
									<div className={styles.keyboardTooltip}>
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
		<Tooltip.Root
			open={!!tooltip}
			disableHoverablePopup={isPointerTransfer}
			onOpenChange={(_open, eventDetails) => {
				eventDetails.allowPropagation();
			}}
		>
			<Tooltip.Trigger render={trigger} />
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8} side="right">
					<Tooltip.Popup render={<TooltipPopup content={tooltip ?? undefined} />} />
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
