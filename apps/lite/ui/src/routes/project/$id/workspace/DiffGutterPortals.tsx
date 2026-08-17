import { Checkbox } from "#ui/components/Checkbox.tsx";
import type { Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { Fragment, memo, type FC, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import styles from "./DiffGutterPortals.module.css";

export type GutterCheckboxGroup = {
	key: string;
	parentOperand: Extract<Operand, { _tag: "Hunk" }>;
	parentSlotName: string;
	lines: Array<{
		operand: Extract<Operand, { _tag: "Hunk" }>;
		slotName: string;
	}>;
};

export type GutterTarget = {
	key: number;
	host: HTMLElement;
	groups: Array<GutterCheckboxGroup>;
};

export type GutterStore = {
	getSnapshot: () => ReadonlyArray<GutterTarget>;
	subscribe: (listener: () => void) => () => void;
};

const LineCheckbox: FC<{
	projectId: string;
	operand: Extract<Operand, { _tag: "Hunk" }>;
	slotName: string;
	onCheck: (operand: Extract<Operand, { _tag: "Hunk" }>, shiftKey: boolean) => void;
}> = (p) => {
	const checked = useAppSelector((state) =>
		projectSlice.selectors.selectOperandChecked(state, p.projectId, p.operand),
	);
	const canCheck = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckHunks(state, p.projectId, p.operand.parent.parent),
	);
	if (!canCheck) return null;

	return (
		<Checkbox
			slot={p.slotName}
			checked={checked}
			onMouseDown={(event) => {
				// Keep the diff selection scope focused so line navigation continues after a click.
				event.preventDefault();
			}}
			onCheckedChange={(_checked, { event }) => {
				const shiftKey =
					(event instanceof MouseEvent || event instanceof KeyboardEvent) &&
					event.shiftKey === true;
				p.onCheck(p.operand, shiftKey);
			}}
			aria-label="Check line"
			className={styles.checkbox}
		/>
	);
};

type HunkCheckedState = "checked" | "indeterminate" | "unchecked";

const HunkCheckbox: FC<{
	projectId: string;
	operand: Extract<Operand, { _tag: "Hunk" }>;
	slotName: string;
	lineOperands: Array<Extract<Operand, { _tag: "Hunk" }>>;
	onCheck: (
		operand: Extract<Operand, { _tag: "Hunk" }>,
		lineOperands: Array<Extract<Operand, { _tag: "Hunk" }>>,
		shiftKey: boolean,
	) => void;
}> = (p) => {
	const checkedState = useAppSelector((state): HunkCheckedState => {
		const checkedCount = p.lineOperands.filter((operand) =>
			projectSlice.selectors.selectOperandChecked(state, p.projectId, operand),
		).length;
		if (checkedCount === 0) return "unchecked";
		return checkedCount === p.lineOperands.length ? "checked" : "indeterminate";
	});
	const canCheck = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckHunks(state, p.projectId, p.operand.parent.parent),
	);
	if (!canCheck) return null;

	return (
		<Checkbox
			slot={p.slotName}
			checked={checkedState === "checked"}
			indeterminate={checkedState === "indeterminate"}
			onMouseDown={(event) => {
				// Keep the diff selection scope focused so line navigation continues after a click.
				event.preventDefault();
			}}
			onCheckedChange={(_checked, { event }) => {
				const shiftKey =
					(event instanceof MouseEvent || event instanceof KeyboardEvent) &&
					event.shiftKey === true;
				p.onCheck(p.operand, p.lineOperands, shiftKey);
			}}
			aria-label="Check hunk"
			className={styles.checkbox}
		/>
	);
};

export const DiffGutterPortals = memo(function DiffGutterPortals({
	projectId,
	store,
	onCheckLine,
	onCheckHunk,
}: {
	projectId: string;
	store: GutterStore;
	onCheckLine: (operand: Extract<Operand, { _tag: "Hunk" }>, shiftKey: boolean) => void;
	onCheckHunk: (
		operand: Extract<Operand, { _tag: "Hunk" }>,
		lineOperands: Array<Extract<Operand, { _tag: "Hunk" }>>,
		shiftKey: boolean,
	) => void;
}) {
	const targets = useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot);

	return targets.map(({ host, key, groups }) =>
		createPortal(
			groups.map((group) => (
				<Fragment key={group.key}>
					<HunkCheckbox
						projectId={projectId}
						operand={group.parentOperand}
						slotName={group.parentSlotName}
						lineOperands={group.lines.map((line) => line.operand)}
						onCheck={onCheckHunk}
					/>
					{group.lines.map((line) => (
						<LineCheckbox
							key={line.slotName}
							projectId={projectId}
							operand={line.operand}
							slotName={line.slotName}
							onCheck={onCheckLine}
						/>
					))}
				</Fragment>
			)),
			host,
			key,
		),
	);
});
