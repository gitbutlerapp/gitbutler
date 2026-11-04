import card from "./card.module.css";
import { classes } from "./classes.ts";
import { ScrollArea } from "./ScrollArea.tsx";
import styles from "./Table.module.css";
import type { ComponentProps, FC, ReactNode } from "react";

/**
 * Things in rows across named columns: API keys, machines, the sounds in a library. The card
 * `DescriptionList` draws, the column names in the `header` row, and a hairline under each row but
 * the last. A row is one thing; its cells hold anything a row shows: text, `Code`, a `Switch`, a
 * ghost `Button`. For names and their values use `DescriptionList`; for settings and choices,
 * `Cell`s in a `RowGroup`.
 *
 * The columns size to what they hold, and a table wider than its card scrolls sideways inside it
 * rather than squeezing them.
 *
 * @public
 * @import import { Table, TableRow, TableCell } from "@gitbutler/ui-react/Table.tsx";
 */
export const Table: FC<{
	/** One `TableRow` of `header` cells: the columns' names. */
	header: ReactNode;
	/** The `TableRow`s. */
	children: ReactNode;
	className?: string;
}> = ({ header, children, className }) => (
	<div className={classes("text-13", card.card, styles.table, className)}>
		<ScrollArea>
			<table className={styles.grid}>
				<thead>{header}</thead>
				<tbody>{children}</tbody>
			</table>
		</ScrollArea>
	</div>
);

/**
 * One row of a {@link Table}: its cells, the same number in every row.
 *
 * @public
 * @import import { Table, TableRow, TableCell } from "@gitbutler/ui-react/Table.tsx";
 */
export const TableRow: FC<ComponentProps<"tr">> = (props) => <tr {...props} />;

/**
 * One cell of a {@link Table}. A `header` cell names its column, in grey; the others hold its
 * values. `align="end"` for numbers (counts, durations, sizes), so they line up on their last digit,
 * and for the actions that end a row; give the column's header cell the same.
 *
 * @public
 * @import import { Table, TableRow, TableCell } from "@gitbutler/ui-react/Table.tsx";
 */
export const TableCell: FC<
	{ header?: boolean; align?: "start" | "end" } & Omit<ComponentProps<"td">, "align">
> = ({ header = false, align = "start", className, ...props }) => {
	const cellClassName = classes(styles.cell, align === "end" && styles.end, className);
	return header ? (
		<th scope="col" {...props} className={classes(cellClassName, styles.header)} />
	) : (
		<td {...props} className={cellClassName} />
	);
};
