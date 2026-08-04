import { classes } from "#ui/components/classes.ts";
import { FieldControlWithIcon, FieldRootStyles } from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { projectSlice } from "#ui/projects/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch } from "#ui/store.ts";
import { Field } from "@base-ui/react";
import type { FC } from "react";
import rowStyles from "../Row.module.css";
import { Row } from "../Row.tsx";
import { getRowButtonClassName } from "../Row-utils.ts";
import styles from "./UncommittedChangesFilterRow.module.css";

/**
 * Takes the place of the uncommitted changes section header while the filter is
 * open, so narrowing the list does not cost a row of vertical space in an
 * already tight panel.
 */
export const UncommittedChangesFilterRow: FC<{
	filter: string;
	projectId: string;
}> = ({ filter, projectId }) => {
	const dispatch = useAppDispatch();

	const setFilter = (value: string | null) => {
		dispatch(projectSlice.actions.setUncommittedFilesFilter({ projectId, filter: value }));
	};

	const closeFilter = () => {
		setFilter(null);
		// Hand focus back to the list the filter was narrowing, rather than
		// dropping it on the body when the input unmounts.
		focusSelectionScope("uncommitted-files");
	};

	return (
		<Row
			interactive={false}
			className={classes(rowStyles.sectionHeader, styles.filterRow)}
			// Escape closes the filter rather than reaching the outline's cancel
			// shortcut, which has nothing to cancel while the input holds focus.
			onKeyDown={(event) => {
				if (event.key !== "Escape") return;

				event.preventDefault();
				event.stopPropagation();
				closeFilter();
			}}
		>
			<Field.Root render={<FieldRootStyles />} className={styles.filterField}>
				<FieldControlWithIcon
					className="text-13"
					icon={<Icon name="search" />}
					aria-label="Filter files"
					placeholder="Filter files"
					value={filter}
					onChange={(event) => setFilter(event.currentTarget.value)}
					// oxlint-disable-next-line jsx_a11y/no-autofocus
					autoFocus
				/>
			</Field.Root>

			<button
				type="button"
				aria-label="Close file filter"
				className={getRowButtonClassName({ size: "regular", iconOnly: true })}
				onClick={closeFilter}
			>
				<Icon name="cross" />
			</button>
		</Row>
	);
};
