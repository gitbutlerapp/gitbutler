import { classes } from "#ui/ui/classes.ts";
import {
	absorbOperation,
	getOperations,
	operationLabel,
	useRunOperationMutationOptions,
	type Operation,
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
import { OperationMode, getBinaryOperation } from "#ui/outline/mode.ts";
import { Match } from "effect";
import { useCommand } from "#ui/commands/manager.ts";
import { useMutation } from "@tanstack/react-query";

const OperationModeControls: FC<{
	projectId: string;
	operation: Operation | null;
}> = ({ projectId, operation }) => {
	const dispatch = useAppDispatch();
	const { mutate: runOperation } = useMutation(useRunOperationMutationOptions());

	const confirm = () => {
		dispatch(projectActions.exitMode({ projectId }));

		if (!operation) return;

		runOperation(operation);
	};

	const cancel = () => dispatch(projectActions.exitMode({ projectId }));

	const confirmCommand = useCommand(confirm, {
		enabled: !!operation,
		group: "Operation mode",
		commandPalette: { label: "Confirm" },
		shortcutsBar: { label: "Confirm" },
		hotkeys: [{ hotkey: "Enter" }],
	});

	const cancelCommand = useCommand(cancel, {
		group: "Operation mode",
		commandPalette: { label: "Cancel" },
		shortcutsBar: { label: "Cancel" },
		hotkeys: [{ hotkey: "Escape" }],
	});

	return (
		<>
			{operation && (
				<ShortcutButton
					className={uiStyles.button}
					hotkeys={confirmCommand.hotkeys}
					onClick={confirmCommand.commandFn}
				>
					{operationLabel(operation)}
				</ShortcutButton>
			)}
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={cancelCommand.hotkeys}
				onClick={cancelCommand.commandFn}
			>
				Cancel
			</ShortcutButton>
		</>
	);
};

const CutOperationControls: FC<{
	projectId: string;
	operations: OperationsByType;
	operationType: OperationType;
}> = ({ projectId, operations, operationType }) => {
	const dispatch = useAppDispatch();
	const { mutate: runOperation } = useMutation(useRunOperationMutationOptions());
	const operation = operations[operationType];

	const run = () => {
		dispatch(projectActions.exitMode({ projectId }));

		if (!operation) return;

		runOperation(operation);
	};

	const cancel = () => dispatch(projectActions.exitMode({ projectId }));

	const setOperationType = (operationType: OperationType) =>
		dispatch(projectActions.updateCutMode({ projectId, operationType }));

	const moveAboveCommand = useCommand(() => setOperationType("moveAbove"), {
		group: "Operation mode",
		commandPalette: operations.moveAbove
			? { label: `Select ${operationLabel(operations.moveAbove)}` }
			: undefined,
		hotkeys: [{ hotkey: "A" }],
	});

	const rubCommand = useCommand(() => setOperationType("rub"), {
		group: "Operation mode",
		commandPalette: operations.rub
			? { label: `Select ${operationLabel(operations.rub)}` }
			: undefined,
		hotkeys: [{ hotkey: "R" }],
	});

	const moveBelowCommand = useCommand(() => setOperationType("moveBelow"), {
		group: "Operation mode",
		commandPalette: operations.moveBelow
			? { label: `Select ${operationLabel(operations.moveBelow)}` }
			: undefined,
		hotkeys: [{ hotkey: "B" }],
	});

	const confirmCommand = useCommand(run, {
		enabled: !!operation,
		group: "Operation mode",
		commandPalette: operation ? { label: operationLabel(operation) } : undefined,
		shortcutsBar: operation ? { label: operationLabel(operation) } : undefined,
		hotkeys: [{ hotkey: "Mod+V", ignoreInputs: true }, { hotkey: "Enter" }],
	});

	const cancelCommand = useCommand(cancel, {
		group: "Operation mode",
		commandPalette: { label: "Cancel" },
		shortcutsBar: { label: "Cancel" },
		hotkeys: [{ hotkey: "Escape" }],
	});

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
					render={<ShortcutButton hotkeys={moveAboveCommand.hotkeys} />}
				>
					{operations.moveAbove ? operationLabel(operations.moveAbove) : "Move above"}
				</Toggle>
				<Toggle
					value={"rub" satisfies OperationType}
					className={styles.operationTypeToggle}
					render={<ShortcutButton hotkeys={rubCommand.hotkeys} />}
				>
					{operations.rub ? operationLabel(operations.rub) : "Rub"}
				</Toggle>
				<Toggle
					value={"moveBelow" satisfies OperationType}
					className={styles.operationTypeToggle}
					render={<ShortcutButton hotkeys={moveBelowCommand.hotkeys} />}
				>
					{operations.moveBelow ? operationLabel(operations.moveBelow) : "Move below"}
				</Toggle>
			</ToggleGroup>
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={confirmCommand.hotkeys}
				onClick={confirmCommand.commandFn}
				disabled={!operation}
			>
				Confirm
			</ShortcutButton>
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={cancelCommand.hotkeys}
				onClick={cancelCommand.commandFn}
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
		operationMode: OperationMode | null;
		isActive: boolean;
	} & useRender.ComponentProps<"div">
> = ({ projectId, target, operationMode, isActive, render, ...props }) => {
	const tooltip =
		isActive && !!operationMode
			? Match.value(operationMode).pipe(
					Match.tagsExhaustive({
						Absorb: ({ absorptionPlan }) => (
							<OperationModeControls
								projectId={projectId}
								operation={absorptionPlan.length > 0 ? absorbOperation({ absorptionPlan }) : null}
							/>
						),
						DragAndDrop: () => {
							const operation = getBinaryOperation({
								mode: operationMode,
								target,
							});
							if (!operation) return null;

							return <>{operationLabel(operation)}</>;
						},
						Cut: ({ source, operationType }) => (
							<>
								{operandEquals(operationMode.source, target) && <>Select a target</>}
								<CutOperationControls
									projectId={projectId}
									operations={getOperations(source, target)}
									operationType={operationType}
								/>
							</>
						),
					}),
				)
			: null;

	const trigger = useRender({ render, props });

	const isDragAndDrop =
		!!operationMode &&
		Match.value(operationMode).pipe(
			Match.tags({ DragAndDrop: () => true }),
			Match.orElse(() => false),
		);

	return (
		<Tooltip.Root
			open={!!tooltip}
			disableHoverablePopup={isDragAndDrop}
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
