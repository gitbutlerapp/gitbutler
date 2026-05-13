import { classes } from "#ui/ui/classes.ts";
import {
	getOperations,
	operationLabel,
	type OperationType,
	type OperationsByType,
} from "#ui/operations/operation.ts";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import uiStyles from "#ui/ui/ui.module.css";
import { Tooltip, useRender } from "@base-ui/react";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import { FC } from "react";
import styles from "./OperationTooltip.module.css";
import { Operand, operandEquals } from "#ui/operands.ts";
import { useAppDispatch } from "#ui/store.ts";
import { projectActions } from "#ui/projects/state.ts";
import { getTransferOperation, type OutlineMode } from "#ui/outline/mode.ts";
import { Match } from "effect";
import { resolvedCommandHotkeys, useProjectCommands } from "#ui/commands/manager.ts";
import { type CommitAbsorption } from "@gitbutler/but-sdk";
import { useFocusedProjectPanel } from "#ui/panels.ts";

const AbsorbControls: FC<{
	projectId: string;
	absorptionPlan: Array<CommitAbsorption>;
}> = ({ projectId, absorptionPlan }) => {
	const focusedPanel = useFocusedProjectPanel(projectId);
	const [cmds, hotkeys] = useProjectCommands({ focusedPanel, projectId });

	return (
		<>
			{absorptionPlan.length > 0 && (
				<ShortcutButton
					className={uiStyles.button}
					hotkeys={resolvedCommandHotkeys(hotkeys["operation.confirm"])}
					onClick={cmds["operation.confirm"]}
				>
					Absorb
				</ShortcutButton>
			)}
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={resolvedCommandHotkeys(hotkeys["mode.cancel"])}
				onClick={cmds["mode.cancel"]}
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
	const operation = operations[operationType];
	const focusedPanel = useFocusedProjectPanel(projectId);
	const [cmds, hotkeys] = useProjectCommands({ focusedPanel, projectId });

	const setOperationType = (operationType: OperationType) =>
		dispatch(projectActions.updateTransferOperationType({ projectId, operationType }));

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
						<ShortcutButton hotkeys={resolvedCommandHotkeys(hotkeys["operation.move_above"])} />
					}
				>
					{operations.moveAbove ? operationLabel(operations.moveAbove) : "Move above"}
				</Toggle>
				<Toggle
					value={"rub" satisfies OperationType}
					className={styles.operationTypeToggle}
					render={<ShortcutButton hotkeys={resolvedCommandHotkeys(hotkeys["operation.rub"])} />}
				>
					{operations.rub ? operationLabel(operations.rub) : "Rub"}
				</Toggle>
				<Toggle
					value={"moveBelow" satisfies OperationType}
					className={styles.operationTypeToggle}
					render={
						<ShortcutButton hotkeys={resolvedCommandHotkeys(hotkeys["operation.move_below"])} />
					}
				>
					{operations.moveBelow ? operationLabel(operations.moveBelow) : "Move below"}
				</Toggle>
			</ToggleGroup>
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={resolvedCommandHotkeys(hotkeys["operation.confirm"])}
				onClick={cmds["operation.confirm"]}
				disabled={!operation}
			>
				Confirm
			</ShortcutButton>
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={resolvedCommandHotkeys(hotkeys["mode.cancel"])}
				onClick={cmds["mode.cancel"]}
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
					Absorb: ({ absorptionPlan }) => (
						<AbsorbControls projectId={projectId} absorptionPlan={absorptionPlan} />
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
					<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip, styles.popup)}>
						{tooltip}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
