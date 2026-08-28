import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Checkbox } from "#ui/components/Checkbox.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import type { Address } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { Fragment, memo, type FC, useEffect, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import type { DiffLineTarget } from "./diff-line-target.ts";
import styles from "./DiffGutterPortals.module.css";

export type GutterCheckboxGroup = {
	key: string;
	parentAddress: Extract<Address, { _tag: "Hunk" }>;
	parentSlotName: string;
	lines: Array<{
		address: Extract<Address, { _tag: "Hunk" }>;
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
	/** Reports whether a hunk holds any checked line, which is what its band stands for. */
	setGroupChecked: (host: HTMLElement, groupKey: string, checked: boolean) => void;
	/** Reports whether the hunk accepts a check at all, since its column answers clicks. */
	setGroupCheckable: (host: HTMLElement, groupKey: string, checkable: boolean) => void;
};

const LineCheckbox: FC<{
	projectId: string;
	address: Extract<Address, { _tag: "Hunk" }>;
	slotName: string;
	onCheck: (address: Extract<Address, { _tag: "Hunk" }>, shiftKey: boolean) => void;
}> = (p) => {
	const checked = useAppSelector((state) =>
		projectSlice.selectors.selectAddressChecked(state, p.projectId, p.address),
	);
	const canCheck = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckHunks(state, p.projectId, p.address.parent.parent),
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
				p.onCheck(p.address, shiftKey);
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
	<span slot={p.slotName} className={styles.comment}>
		<button
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
			className={getButtonClassName({ variant: "ghost", size: "small", iconOnly: true })}
		>
			<Icon name="plus" />
		</button>
	</span>
);

const HunkCheckbox: FC<{
	projectId: string;
	address: Extract<Address, { _tag: "Hunk" }>;
	slotName: string;
	lineAddresses: Array<Extract<Address, { _tag: "Hunk" }>>;
	onCheck: (
		address: Extract<Address, { _tag: "Hunk" }>,
		lineAddresses: Array<Extract<Address, { _tag: "Hunk" }>>,
		shiftKey: boolean,
	) => void;
	store: GutterStore;
	host: HTMLElement;
	groupKey: string;
}> = (p) => {
	const checkedState = useAppSelector((state): HunkCheckedState => {
		const checkedCount = p.lineAddresses.filter((address) =>
			projectSlice.selectors.selectAddressChecked(state, p.projectId, address),
		).length;
		if (checkedCount === 0) return "unchecked";
		return checkedCount === p.lineAddresses.length ? "checked" : "indeterminate";
	});
	const canCheck = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckHunks(state, p.projectId, p.address.parent.parent),
	);
	// The band spans lines this checkbox does not stand on, and answers clicks along all of them, so
	// the store learns both what to paint and whether the act is available at all from here. The
	// store, host and key are what the reports are addressed to, so they are read off p rather than
	// handed in as closures, which would be new every render and make each report run twice.
	const { store, host, groupKey } = p;
	useEffect(() => {
		store.setGroupChecked(host, groupKey, checkedState !== "unchecked");
		return () => store.setGroupChecked(host, groupKey, false);
	}, [store, host, groupKey, checkedState]);
	useEffect(() => {
		store.setGroupCheckable(host, groupKey, canCheck);
		return () => store.setGroupCheckable(host, groupKey, false);
	}, [store, host, groupKey, canCheck]);
	if (!canCheck) return null;

	return (
		<Checkbox
			slot={p.slotName}
			className={classes(styles.checkbox, styles.hunkCheckbox)}
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
				p.onCheck(p.address, p.lineAddresses, shiftKey);
			}}
			aria-label="Check hunk"
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
	onCheckLine: (address: Extract<Address, { _tag: "Hunk" }>, shiftKey: boolean) => void;
	onCheckHunk: (
		address: Extract<Address, { _tag: "Hunk" }>,
		lineAddresses: Array<Extract<Address, { _tag: "Hunk" }>>,
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
							address={group.parentAddress}
							slotName={group.parentSlotName}
							lineAddresses={group.lines.map((line) => line.address)}
							onCheck={onCheckHunk}
							store={store}
							host={host}
							groupKey={group.key}
						/>
						{group.lines.map((line) => (
							<LineCheckbox
								key={line.slotName}
								projectId={projectId}
								address={line.address}
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
