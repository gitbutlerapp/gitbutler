import { useAbsorb } from "#ui/api/mutations.ts";
import { absorptionPlanQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex, type HeadInfoIndex } from "#ui/api/ref-info.ts";
import { assert } from "#ui/assert.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { Kbd } from "#ui/components/Kbd.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { operationHotkeys } from "#ui/hotkeys.ts";
import { Operand } from "#ui/operands.ts";
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
import { Button, Tooltip } from "@base-ui/react";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import { useHotkeys, type UseHotkeyDefinition } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match } from "effect";
import { FC, type ReactNode, use } from "react";
import styles from "./OperationControls.module.css";
import { AbsorbMode, KeyboardTransferOperationMode } from "#ui/outline/mode.ts";

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
		{ hotkey: operationHotkeys.confirm.hotkey, options: { meta: operationHotkeys.confirm.meta } },
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
				meta: operationHotkeys.cancel.meta,
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
								{operationHotkeys.confirm.meta.name}
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
							{operationHotkeys.cancel.meta.name}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
		</div>
	);
};

const CheckedCommitOperationControls: FC<{ checkedCommitCount: number; projectId: string }> = ({
	checkedCommitCount,
	projectId,
}) => {
	const dispatch = useAppDispatch();

	const cancel = () => {
		dispatch(projectActions.clearCheckedCommits({ projectId }));
		focusSelectionScope("outline");
	};

	return (
		<Container>
			<ControlsRow>
				<Label>
					{new Intl.NumberFormat().format(checkedCommitCount)}{" "}
					{new Intl.PluralRules().select(checkedCommitCount) === "one" ? "commit" : "commits"}{" "}
					checked
				</Label>
				<Controls onCancel={cancel} />
			</ControlsRow>
		</Container>
	);
};

const AbsorbOperationControls: FC<{
	headInfoIndex: HeadInfoIndex;
	projectId: string;
	mode: AbsorbMode;
}> = ({ headInfoIndex, projectId, mode }) => {
	const dispatch = useAppDispatch();
	const absorptionPlan = useQuery(
		absorptionPlanQueryOptions({ projectId, target: mode.sourceTarget }),
	);
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

	return (
		<Container>
			<ControlsRow>
				{absorptionPlan.isPending ? (
					<Icon name="spinner" aria-label="Loading absorb plan" />
				) : absorptionPlan.isError ? (
					<Label>Failed to load absorb plan</Label>
				) : (
					<Label>
						Absorb {operandLabel({ headInfoIndex, operand: mode.source })} into{" "}
						{absorptionPlan.data.length} commits
					</Label>
				)}
				<Controls onCancel={cancel} confirm={{ canRun: canAbsorb, onRun: confirm }} />
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
			hotkey: operationHotkeys.selectInto.hotkey,
			callback: () => setOperationType("into"),
			options: {
				conflictBehavior: "allow",
				meta: operationHotkeys.selectInto.meta,
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
			onMouseDown={(event) => {
				// Prevent stealing focus from the tree.
				if (!event.defaultPrevented) event.preventDefault();
			}}
		>
			<Toggle className={styles.toggleGroupRowToggle} value={"above" satisfies OperationType}>
				{operations.above && (
					<div className={classes("text-12", styles.operationLabel)}>{operations.above.label}</div>
				)}
				<div className="text-semibold">
					Above <Kbd hotkey={operationHotkeys.selectAbove.hotkey} />
				</div>
			</Toggle>

			<Toggle className={styles.toggleGroupRowToggle} value={"into" satisfies OperationType}>
				{operations.into && (
					<div className={classes("text-12", styles.operationLabel)}>{operations.into.label}</div>
				)}
				<div className="text-semibold">
					Into <Kbd hotkey={operationHotkeys.selectInto.hotkey} />
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

const TransferKeyboardOperationControls: FC<{
	headInfoIndex: HeadInfoIndex;
	projectId: string;
	mode: KeyboardTransferOperationMode;
	target: Operand;
}> = ({ headInfoIndex, projectId, mode, target }) => {
	const dispatch = useAppDispatch();
	const { mutate: runOperation } = useRunOperation();
	const operations = getOperations(mode.source, target);
	const operation = operations[mode.operationType];

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
					<div>Source: {operandLabel({ headInfoIndex, operand: mode.source })}</div>
					<div>Target: {operandLabel({ headInfoIndex, operand: target })}</div>
				</Label>
				<Controls
					onCancel={cancel}
					confirm={{
						canRun: !!operation,
						onRun: run,
						extraHotkeys: [
							{
								hotkey: operationHotkeys.confirmTransfer.hotkey,
								options: { meta: operationHotkeys.confirmTransfer.meta, ignoreInputs: true },
							},
						],
					}}
				/>
			</ControlsRow>
		</Container>
	);
};

export const OperationControls: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const navigationIndex = assert(use(NavigationIndexContext));
	const selection = useOutlineSelection({ projectId, navigationIndex });
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const checkedCommitCount = useAppSelector((state) =>
		selectProjectCheckedCommitCount(state, projectId),
	);

	return Match.value(outlineMode).pipe(
		Match.tagsExhaustive({
			Default: () =>
				checkedCommitCount > 0 && (
					<CheckedCommitOperationControls
						checkedCommitCount={checkedCommitCount}
						projectId={projectId}
					/>
				),
			Absorb: (mode) =>
				headInfoIndex && (
					<AbsorbOperationControls
						headInfoIndex={headInfoIndex}
						projectId={projectId}
						mode={mode}
					/>
				),
			Transfer: ({ value: mode }) =>
				Match.value(mode).pipe(
					Match.tags({
						Keyboard: (mode) =>
							selection &&
							headInfoIndex && (
								<TransferKeyboardOperationControls
									headInfoIndex={headInfoIndex}
									projectId={projectId}
									mode={mode}
									target={selection}
								/>
							),
					}),
					Match.orElse(() => null),
				),
			RenameBranch: () => null,
			RewordCommit: () => null,
		}),
	);
};
