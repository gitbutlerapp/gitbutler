import { useAbsorb } from "#ui/api/mutations.ts";
import { absorptionPlanQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { assert } from "#ui/assert.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { operationHotkeys } from "#ui/hotkeys.ts";
import {
	getOperations,
	useRunOperation,
	type OperationType,
	type OperationsByType,
} from "#ui/operations/operation.ts";
import {
	projectActions,
	selectProjectCheckedCommitCount,
	selectProjectOutlineModeState,
} from "#ui/projects/state.ts";
import { NavigationIndexContext } from "#ui/routes/project/$id/workspace/OutlineNavigationIndexContext.ts";
import { operandLabel } from "#ui/routes/project/$id/workspace/operandLabel.ts";
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
import { Kbd } from "#ui/components/Kbd.tsx";

const Container: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={classes("text-14", styles.container)}>{children}</div>
);

const ControlsRow: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={styles.controlsRow}>{children}</div>
);

const Label: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={classes(styles.label, "text-bold", "text-13")}>{children}</div>
);

const Controls: FC<{ children: ReactNode }> = ({ children }) => (
	<div className={styles.controls}>{children}</div>
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
		<Controls>
			<Tooltip.Root>
				<Tooltip.Trigger
					className={getButtonClassName({ variant: "gray" })}
					onClick={confirm}
					// We pass `disabled` here because we want to disable the button, not
					// the tooltip. Other props should be passed above.
					render={<Button focusableWhenDisabled disabled={!canAbsorb} />}
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
		</Controls>
	);
};

const CheckedCommitControls: FC<{ checkedCommitCount: number; projectId: string }> = ({
	checkedCommitCount,
	projectId,
}) => {
	const dispatch = useAppDispatch();

	const cancel = () => {
		dispatch(projectActions.clearCheckedCommits({ projectId }));
		focusSelectionScope("outline");
	};

	useHotkeys([
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
		<Container>
			<ControlsRow>
				<Label>
					{new Intl.NumberFormat().format(checkedCommitCount)}{" "}
					{new Intl.PluralRules().select(checkedCommitCount) === "one" ? "commit" : "commits"}{" "}
					checked
				</Label>
				<Controls>
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
				</Controls>
			</ControlsRow>
		</Container>
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
			hotkey: operationHotkeys.selectAbove.hotkey,
			callback: () => setOperationType("above"),
			options: {
				conflictBehavior: "allow",
				meta: operationHotkeys.selectAbove.meta,
			},
		},
		{
			hotkey: operationHotkeys.selectCombine.hotkey,
			callback: () => setOperationType("combine"),
			options: {
				conflictBehavior: "allow",
				meta: operationHotkeys.selectCombine.meta,
			},
		},
		{
			hotkey: operationHotkeys.selectBelow.hotkey,
			callback: () => setOperationType("below"),
			options: {
				conflictBehavior: "allow",
				meta: operationHotkeys.selectBelow.meta,
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
			aria-label="Operation type"
			value={[operationType]}
			onValueChange={onValueChange}
			className={styles.toggleGroupRow}
		>
			<Toggle className={styles.toggleGroupRowToggle} value={"above" satisfies OperationType}>
				{operations.above && (
					<div className={classes("text-12", styles.operationLabel)}>{operations.above.label}</div>
				)}
				<div className="text-semibold">
					Above <Kbd hotkey={operationHotkeys.selectAbove.hotkey} />
				</div>
			</Toggle>

			<Toggle className={styles.toggleGroupRowToggle} value={"combine" satisfies OperationType}>
				{operations.combine && (
					<div className={classes("text-12", styles.operationLabel)}>
						{operations.combine.label}
					</div>
				)}
				<div className="text-semibold">
					Combine <Kbd hotkey={operationHotkeys.selectCombine.hotkey} />
				</div>
			</Toggle>

			<Toggle className={styles.toggleGroupRowToggle} value={"below" satisfies OperationType}>
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

		runOperation(operation.operation);
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
		<Controls>
			<Tooltip.Root>
				<Tooltip.Trigger
					className={getButtonClassName({ variant: "gray" })}
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
		</Controls>
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
	const checkedCommitCount = useAppSelector((state) =>
		selectProjectCheckedCommitCount(state, projectId),
	);

	return Match.value(outlineMode).pipe(
		Match.tagsExhaustive({
			Default: () =>
				checkedCommitCount > 0 && (
					<CheckedCommitControls checkedCommitCount={checkedCommitCount} projectId={projectId} />
				),
			Absorb: (x) =>
				absorptionPlanQuery &&
				headInfo && (
					<Container>
						<ControlsRow>
							{absorptionPlanQuery.isPending ? (
								<Icon name="spinner" aria-label="Loading absorb plan" />
							) : absorptionPlanQuery.isError ? (
								<Label>Failed to load absorb plan</Label>
							) : (
								<Label>
									Absorb {operandLabel({ headInfo, operand: x.source })} into{" "}
									{absorptionPlanQuery.data.length} commits
								</Label>
							)}
							<AbsorbControls projectId={projectId} sourceTarget={x.sourceTarget} />
						</ControlsRow>
					</Container>
				),
			Transfer: ({ value: mode }) =>
				Match.value(mode).pipe(
					Match.tags({
						Keyboard: (mode) =>
							selection &&
							headInfo &&
							(() => {
								const operations = getOperations(mode.source, selection);
								return (
									<Container>
										<TransferTypeToggleGroup
											projectId={projectId}
											operations={operations}
											operationType={mode.operationType}
										/>
										<Separator />
										<ControlsRow>
											<Label>
												<div>Source: {operandLabel({ headInfo, operand: mode.source })}</div>
												<div>Target: {operandLabel({ headInfo, operand: selection })}</div>
											</Label>
											<TransferOperationControls
												projectId={projectId}
												operations={operations}
												operationType={mode.operationType}
											/>
										</ControlsRow>
									</Container>
								);
							})(),
					}),
					Match.orElse(() => null),
				),
			RenameBranch: () => null,
			RewordCommit: () => null,
		}),
	);
};
