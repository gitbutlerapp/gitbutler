import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Checkbox } from "#ui/components/Checkbox.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import type { Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { Fragment, memo, type FC, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import type { DiffLineTarget } from "./diff-line-target.ts";
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
	comment: {
		slotName: string;
		getTarget: () => DiffLineTarget | undefined;
	};
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

const CommentButton: FC<{
	slotName: string;
	getTarget: () => DiffLineTarget | undefined;
	onComment: (target: DiffLineTarget) => void;
}> = (p) => (
	<button
		slot={p.slotName}
		type="button"
		onPointerDown={(event) => {
			// This control lives inside a line-number cell, but pressing it is not a line selection.
			event.preventDefault();
			event.stopPropagation();
		}}
		onClick={(event) => {
			event.stopPropagation();
			const target = p.getTarget();
			if (target) p.onComment(target);
		}}
		aria-label="Annotate"
		className={classes(
			getButtonClassName({ variant: "pop", size: "small", iconOnly: true }),
			styles.comment,
		)}
	>
		<Icon name="plus" />
	</button>
);

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
	onComment,
}: {
	projectId: string;
	store: GutterStore;
	onCheckLine: (operand: Extract<Operand, { _tag: "Hunk" }>, shiftKey: boolean) => void;
	onCheckHunk: (
		operand: Extract<Operand, { _tag: "Hunk" }>,
		lineOperands: Array<Extract<Operand, { _tag: "Hunk" }>>,
		shiftKey: boolean,
	) => void;
	onComment?: (target: DiffLineTarget) => void;
}) {
	const targets = useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot);

	return targets.map(({ host, key, groups, comment }) =>
		createPortal(
			<>
				{onComment && (
					<CommentButton
						slotName={comment.slotName}
						getTarget={comment.getTarget}
						onComment={onComment}
					/>
				)}
				{groups.map((group) => (
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
				))}
			</>,
			host,
			key,
		),
	);
});
