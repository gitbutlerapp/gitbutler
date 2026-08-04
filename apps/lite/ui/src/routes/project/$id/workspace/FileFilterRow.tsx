import { classes } from "#ui/components/classes.ts";
import { FieldControlWithIcon, FieldRootStyles } from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Field } from "@base-ui/react";
import type { FC } from "react";
import rowStyles from "./Row.module.css";
import { Row } from "./Row.tsx";
import { getRowButtonClassName } from "./Row-utils.ts";
import styles from "./FileFilterRow.module.css";

/**
 * Takes the place of a file list's section header while its filter is open, so
 * narrowing the list does not cost a row of vertical space in an already tight
 * panel. Pair it with `useFileFilter`, which supplies these props and binds the
 * keys that lead in and out of the input.
 */
export const FileFilterRow: FC<{
	filter: string;
	inputId: string;
	onFilterChange: (filter: string) => void;
	onClose: () => void;
	/** Moves focus down into the filtered list, so a match can be previewed without the mouse. */
	onEnterList: () => void;
}> = ({ filter, inputId, onFilterChange, onClose, onEnterList }) => (
	<Row
		interactive={false}
		className={classes(rowStyles.sectionHeader, rowStyles.sectionHeaderFlush, styles.filterRow)}
		onKeyDown={(event) => {
			// Escape closes the filter rather than reaching the outline's cancel
			// shortcut, which has nothing to cancel while the input holds focus.
			if (event.key === "Escape") {
				event.preventDefault();
				event.stopPropagation();
				onClose();
				return;
			}

			// Down leaves the input for the list it filters, the way it would step
			// between rows there. The filter stays open and keeps its query, so the
			// list can be walked and narrowed in turn.
			if (event.key === "ArrowDown") {
				event.preventDefault();
				event.stopPropagation();
				onEnterList();
			}
		}}
	>
		<Field.Root render={<FieldRootStyles />} className={styles.filterField}>
			<FieldControlWithIcon
				id={inputId}
				className="text-13"
				icon={<Icon name="search" />}
				aria-label="Filter files"
				placeholder="Filter files"
				value={filter}
				onChange={(event) => onFilterChange(event.currentTarget.value)}
				// oxlint-disable-next-line jsx_a11y/no-autofocus
				autoFocus
			/>
		</Field.Root>

		<button
			type="button"
			aria-label="Close file filter"
			className={getRowButtonClassName({ size: "regular", iconOnly: true })}
			onClick={onClose}
		>
			<Icon name="cross" />
		</button>
	</Row>
);
