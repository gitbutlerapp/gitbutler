import { useAbsorb } from "#ui/api/mutations.ts";
import { cancelMode, useResolvedCursor, useWorkspaceList } from "#ui/use-cursor.ts";
import { absorptionPlanQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex, type HeadInfoIndex } from "#ui/api/ref-info.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { Kbd } from "#ui/components/Kbd.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { operationHotkeys } from "#ui/hotkeys.ts";
import type { Operand } from "#ui/operands.ts";
import {
	getOperations,
	useExecuteOperation,
	type Placement,
	type OperationsByPlacement,
} from "#ui/operations/operation.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { operandLabel, operandsLabel } from "#ui/routes/project/$id/workspace/operandLabel.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Button, Toggle, ToggleGroup, Tooltip } from "@base-ui/react";
import { useHotkeys, type UseHotkeyDefinition } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match } from "effect";
import type { FC, ReactNode } from "react";
import styles from "./OperationControls.module.css";
import {
	type AbsorbMode,
	getTransferTarget,
	type KeyboardTransferMode,
	keyboardTransferMode,
} from "#ui/outline/mode.ts";
import type { NavigationIndex } from "#ui/workspace/navigation-index.ts";

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

const CheckedOperandOperationControls: FC<{ checkedOperandCount: number; projectId: string }> = ({
	checkedOperandCount,
	projectId,
}) => {
	const dispatch = useAppDispatch();

	const checkedType = useAppSelector((state): string | null => {
		switch (projectSlice.selectors.selectCheckedOperandsContext(state, projectId)) {
			case "Commit":
				return "commit";
			case "File":
				return "file";
			case "Hunk":
				return "hunk";
			case null:
				return null;
		}
	});
	if (checkedType === null) return;

	const cancel = () => {
		dispatch(projectSlice.actions.clearCheckedOperands({ projectId }));
	};

	return (
		<Container>
			<ControlsRow>
				<Label>
					{new Intl.NumberFormat().format(checkedOperandCount)} {checkedType}
					{new Intl.PluralRules().select(checkedOperandCount) !== "one" && "s"} selected
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
	const {
		data: absorptionPlan,
		isError: isAbsorptionPlanError,
		isPending: isAbsorptionPlanPending,
	} = useQuery(absorptionPlanQueryOptions({ projectId, target: mode.sourceTarget }));
	const canAbsorb = !isAbsorptionPlanPending && !!absorptionPlan && absorptionPlan.length > 0;
	const { mutate: absorb } = useAbsorb({ projectId });

	const run = () => {
		dispatch(projectSlice.actions.exitMode({ projectId }));

		absorb(absorptionPlan);
	};

	const cancel = () => {
		cancelMode();
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
						Absorb {operandLabel({ headInfoIndex, operand: mode.source })} into{" "}
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
			},
		},
		{
			hotkey: operationHotkeys.selectInto.hotkey,
			callback: () => setPlacement("into"),
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: operationHotkeys.selectBelow.hotkey,
			callback: () => setPlacement("below"),
			options: {
				conflictBehavior: "allow",
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
			aria-label="Operation type"
			value={[placement]}
			onValueChange={onValueChange}
			className={styles.toggleGroupRow}
			onMouseDown={(event) => {
				// Prevent stealing focus from the tree.
				if (!event.defaultPrevented) event.preventDefault();
			}}
		>
			<Toggle className={styles.toggleGroupRowToggle} value={"above" satisfies Placement}>
				{operations.above && (
					<div className={classes("text-12", styles.operationLabel)}>{operations.above.label}</div>
				)}
				<div className="text-semibold">
					Above <Kbd hotkey={operationHotkeys.selectAbove.hotkey} />
				</div>
			</Toggle>

			<Toggle className={styles.toggleGroupRowToggle} value={"into" satisfies Placement}>
				{operations.into && (
					<div className={classes("text-12", styles.operationLabel)}>{operations.into.label}</div>
				)}
				<div className="text-semibold">
					Into <Kbd hotkey={operationHotkeys.selectInto.hotkey} />
				</div>
			</Toggle>

			<Toggle className={styles.toggleGroupRowToggle} value={"below" satisfies Placement}>
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
	mode: KeyboardTransferMode;
	outlineNavigationIndex: NavigationIndex<Operand>;
}> = ({ headInfoIndex, projectId, mode, outlineNavigationIndex }) => {
	const workspaceList = useWorkspaceList();
	const selection = useResolvedCursor("stacks", outlineNavigationIndex);

	const dispatch = useAppDispatch();
	const { mutate: executeOperation } = useExecuteOperation();

	const target = getTransferTarget(keyboardTransferMode(mode), selection, workspaceList);
	if (!target) return null;

	const operations = getOperations(mode.sources, target);
	const operation = operations[mode.placement];

	const run = () => {
		dispatch(projectSlice.actions.exitMode({ projectId }));

		if (!operation) return;

		executeOperation(operation.operation);
	};

	const cancel = () => {
		cancelMode();
	};

	return (
		<Container>
			<TransferTypeToggleGroup
				projectId={projectId}
				operations={operations}
				placement={mode.placement}
			/>
			<Separator />
			<ControlsRow>
				<Label>
					<div>Source: {operandsLabel({ headInfoIndex, operands: mode.sources })}</div>
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
							},
						],
					}}
				/>
			</ControlsRow>
		</Container>
	);
};

export const OperationControls: FC<{ outlineNavigationIndex: NavigationIndex<Operand> }> = ({
	outlineNavigationIndex,
}) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const outlineMode = useAppSelector((state) =>
		projectSlice.selectors.selectOutlineModeState(state, projectId),
	);
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const checkedOperandCount = useAppSelector((state) =>
		projectSlice.selectors.selectCheckedOperandCount(state, projectId),
	);

	return Match.value(outlineMode).pipe(
		Match.tagsExhaustive({
			Default: () =>
				checkedOperandCount > 0 && (
					<CheckedOperandOperationControls
						checkedOperandCount={checkedOperandCount}
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
							headInfoIndex && (
								<TransferKeyboardOperationControls
									headInfoIndex={headInfoIndex}
									projectId={projectId}
									mode={mode}
									outlineNavigationIndex={outlineNavigationIndex}
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
