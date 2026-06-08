import { useAbsorb } from "#ui/api/mutations.ts";
import { absorptionPlanQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { assert } from "#ui/assert.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { operationHotkeys } from "#ui/hotkeys.ts";
import {
	getOperations,
	operationLabel,
	useRunOperation,
	type OperationType,
	type OperationsByType,
} from "#ui/operations/operation.ts";
import { projectActions, selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { NavigationIndexContext } from "#ui/routes/project/$id/workspace/OutlineNavigationIndexContext.ts";
import { operationSourceLabel } from "#ui/routes/project/$id/workspace/operationSourceLabel.ts";
import { focusSelectionScope, useOutlineSelection } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { Button, Tooltip } from "@base-ui/react";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import { type AbsorptionTarget } from "@gitbutler/but-sdk";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useQueries, useQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match } from "effect";
import { FC, type ReactNode, use } from "react";
import styles from "./OperationControls.module.css";

const Container: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={classes("text-14", styles.container)}>{children}</div>
);

const Separator: FC = () => <div className={styles.separator} />;

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
		focusSelectionScope("outline");

		absorbMutation.mutate(absorptionPlan.data);
	};

	const cancel = () => {
		dispatch(projectActions.cancelMode({ projectId }));
		focusSelectionScope("outline");
	};

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
		focusSelectionScope("outline");
	};

	return (
		<ToggleGroup
			render={<ToggleGroupStyles />}
			aria-label="Operation type"
			value={[operationType]}
			onValueChange={onValueChange}
			className={styles.toggleGroupRow}
		>
			<Tooltip.Root>
				<Toggle
					value={"moveAbove" satisfies OperationType}
					render={<Tooltip.Trigger render={<ToggleStyles />} />}
				>
					{operations.moveAbove ? operationLabel(operations.moveAbove) : "Move above"}
				</Toggle>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
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
					<Tooltip.Positioner sideOffset={4}>
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
					<Tooltip.Positioner sideOffset={4}>
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
		focusSelectionScope("outline");

		if (!operation) return;

		runOperation(operation);
	};

	const cancel = () => {
		dispatch(projectActions.cancelMode({ projectId }));
		focusSelectionScope("outline");
	};

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

export const OperationControls: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const navigationIndex = assert(use(NavigationIndexContext));
	const selection = useOutlineSelection({ projectId, navigationIndex });
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const absorptionPlanTarget = Match.value(outlineMode).pipe(
		Match.tag("Absorb", ({ sourceTarget }) => sourceTarget),
		Match.orElse(() => null),
	);
	const [absorptionPlanQuery] = useQueries({
		queries: (absorptionPlanTarget ? [absorptionPlanTarget] : []).map((target) =>
			absorptionPlanQueryOptions({ projectId, target }),
		),
	});

	if (!headInfo) return null;

	return Match.value(outlineMode).pipe(
		Match.tagsExhaustive({
			Default: () => null,
			Absorb: (x) =>
				absorptionPlanQuery && (
					<Container>
						<div className={styles.controlsRow}>
							<div className={classes("text-bold", "text-13")}>
								{operationSourceLabel({ headInfo, source: x.source })}
							</div>
							{absorptionPlanQuery.isPending && (
								<Icon name="spinner" aria-label="Loading absorb plan" />
							)}
							<AbsorbControls projectId={projectId} sourceTarget={x.sourceTarget} />
						</div>
					</Container>
				),
			Transfer: ({ value: mode }) =>
				Match.value(mode).pipe(
					Match.tags({
						Keyboard: (mode) =>
							selection && (
								<Container>
									<TransferTypeToggleGroup
										projectId={projectId}
										operations={getOperations(mode.source, selection)}
										operationType={mode.operationType}
									/>
									<Separator />
									<div className={styles.controlsRow}>
										<div className={classes("text-bold", "text-13")}>
											{operationSourceLabel({ headInfo, source: mode.source })}
										</div>
										<TransferOperationControls
											projectId={projectId}
											operations={getOperations(mode.source, selection)}
											operationType={mode.operationType}
										/>
									</div>
								</Container>
							),
					}),
					Match.orElse(() => null),
				),
			RenameBranch: () => null,
			RewordCommit: () => null,
		}),
	);
};
