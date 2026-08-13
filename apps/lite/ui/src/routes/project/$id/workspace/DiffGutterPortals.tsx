import type { Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { memo, type FC, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import styles from "./DiffGutterPortals.module.css";

export type GutterCheckboxGroup = {
	key: string;
	operand: Extract<Operand, { _tag: "Hunk" }>;
	slotNames: Array<string>;
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

const HunkCheckboxes: FC<{
	projectId: string;
	operand: Extract<Operand, { _tag: "Hunk" }>;
	slotNames: Array<string>;
	onCheck: (event: { operand: Extract<Operand, { _tag: "Hunk" }>; shiftKey: boolean }) => void;
}> = (p) => {
	const checked = useAppSelector((state) =>
		projectSlice.selectors.selectOperandChecked(state, p.projectId, p.operand),
	);
	const canCheck = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckHunks(state, p.projectId, p.operand.parent.parent),
	);
	if (!canCheck) return null;

	return p.slotNames.map((slotName, index) => (
		<input
			key={slotName}
			type="checkbox"
			slot={slotName}
			checked={checked}
			tabIndex={index === 0 ? 0 : -1}
			onMouseDown={(event) => {
				// Keep the diff selection scope focused so hunk navigation continues after a click.
				event.preventDefault();
			}}
			onChange={(event) => {
				const nativeEvent = event.nativeEvent;
				const shiftKey =
					(nativeEvent instanceof MouseEvent || nativeEvent instanceof KeyboardEvent) &&
					nativeEvent.shiftKey === true;
				p.onCheck({ operand: p.operand, shiftKey });
			}}
			aria-label="Select hunk"
			className={styles.checkbox}
		/>
	));
};

export const DiffGutterPortals = memo(function DiffGutterPortals({
	projectId,
	store,
	onCheck,
}: {
	projectId: string;
	store: GutterStore;
	onCheck: (event: { operand: Extract<Operand, { _tag: "Hunk" }>; shiftKey: boolean }) => void;
}) {
	const targets = useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot);

	return targets.map(({ host, key, groups }) =>
		createPortal(
			groups.map((group) => (
				<HunkCheckboxes
					key={group.key}
					projectId={projectId}
					operand={group.operand}
					slotNames={group.slotNames}
					onCheck={onCheck}
				/>
			)),
			host,
			key,
		),
	);
});
