import { useAbsorb } from "#ui/api/mutations.ts";
import { absorptionPlanQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex, type HeadInfoIndex } from "#ui/api/ref-info.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { CheckedCommitIdsContext } from "#ui/CheckedCommitIdsContext.ts";
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
import { operandLabel } from "#ui/routes/project/$id/workspace/operandLabel.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { OutlineModeContext, OutlineSelectionContext } from "#ui/WorkspaceContext.ts";
import { resolveOutlineSelection } from "#ui/workspace.ts";
import { Button, Tooltip } from "@base-ui/react";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import { useHotkeys, type UseHotkeyDefinition } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match } from "effect";
import { FC, type ReactNode, use } from "react";
import styles from "./OperationControls.module.css";
import { AbsorbMode, KeyboardTransferMode } from "#ui/outline/mode.ts";
import { NavigationIndex } from "#ui/workspace/navigation-index.ts";

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

const CheckedCommitOperationControls: FC<{ checkedCommitCount: number; projectId: string }> = ({
	checkedCommitCount,
	projectId,
}) => {
	const { clearCheckedCommits } = use(CheckedCommitIdsContext);

	const cancel = () => {
		clearCheckedCommits(projectId);
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
	const { exitMode, cancelMode } = use(OutlineModeContext);
	const absorptionPlan = useQuery(
		absorptionPlanQueryOptions({ projectId, target: mode.sourceTarget }),
	);
	const canAbsorb =
		!absorptionPlan.isPending && !!absorptionPlan.data && absorptionPlan.data.length > 0;
	const absorbMutation = useAbsorb({ projectId });

	const run = () => {
		exitMode(projectId);
		focusSelectionScope("outline");

		absorbMutation.mutate(absorptionPlan.data);
	};

	const cancel = () => {
		cancelMode(projectId);
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
				<Controls onCancel={cancel} confirm={{ canRun: canAbsorb, onRun: run }} />
			</ControlsRow>
		</Container>
	);
};

const TransferTypeToggleGroup: FC<{
	operations: OperationsByType;
	operationType: OperationType;
	projectId: string;
}> = ({ operations, operationType, projectId }) => {
	const { updateTransferOperationType } = use(OutlineModeContext);

	const setOperationType = (operationType: OperationType) =>
		updateTransferOperationType(projectId, operationType);

	useHotkeys([
		{
			hotkey: operationHotkeys.selectAbove.hotkey,
			callback: () => setOperationType("above"),
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: operationHotkeys.selectInto.hotkey,
			callback: () => setOperationType("into"),
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: operationHotkeys.selectBelow.hotkey,
			callback: () => setOperationType("below"),
			options: {
				conflictBehavior: "allow",
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
	mode: KeyboardTransferMode;
	outlineNavigationIndex: NavigationIndex<Operand>;
	projectId: string;
}> = ({ headInfoIndex, mode, outlineNavigationIndex, projectId }) => {
	const { outlineSelection } = use(OutlineSelectionContext);
	const { exitMode, cancelMode } = use(OutlineModeContext);
	const selection = resolveOutlineSelection(outlineSelection, outlineNavigationIndex);
	const { mutate: runOperation } = useRunOperation();

	if (!selection) return null;

	const target = selection;

	const operations = getOperations(mode.source, target);
	const operation = operations[mode.operationType];

	const run = () => {
		exitMode(projectId);
		focusSelectionScope("outline");

		if (!operation) return;

		runOperation(operation.operation);
	};

	const cancel = () => {
		cancelMode(projectId);
		focusSelectionScope("outline");
	};

	return (
		<Container>
			<TransferTypeToggleGroup
				operations={operations}
				operationType={mode.operationType}
				projectId={projectId}
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
	const { checkedCommitIds } = use(CheckedCommitIdsContext);
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const { outlineMode } = use(OutlineModeContext);
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const checkedCommitCount = checkedCommitIds.size;

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
							headInfoIndex && (
								<TransferKeyboardOperationControls
									headInfoIndex={headInfoIndex}
									mode={mode}
									outlineNavigationIndex={outlineNavigationIndex}
									projectId={projectId}
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
