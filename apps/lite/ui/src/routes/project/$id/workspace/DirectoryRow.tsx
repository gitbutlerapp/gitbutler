import { Icon } from "#ui/components/Icon.tsx";
import { classes } from "#ui/components/classes.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import type { ComponentProps, FC } from "react";
import styles from "./FilesTree.module.css";
import rowStyles from "./Row.module.css";
import { Row, RowCheckbox, RowLabel, RowLabelContainer } from "./Row.tsx";

/** Whether every file below a directory is checked, some of them, or none. */
export type DirectoryCheckedState = "checked" | "indeterminate" | "unchecked";

export const DirectoryRow: FC<
	{
		projectId: string;
		path: string;
		/** The trailing path segments this row stands for, e.g. `src/lib`. */
		name: string;
		fileCount: number;
		isCollapsed: boolean;
		canCheck: boolean;
		checkedState: DirectoryCheckedState;
		checkDirectory: (evt: { path: string; checked: boolean }) => void;
	} & ComponentProps<typeof Row>
> = ({
	projectId,
	path,
	name,
	fileCount,
	isCollapsed,
	canCheck,
	checkedState,
	checkDirectory,
	...restProps
}) => {
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);

	return (
		<Row
			{...restProps}
			isChecked={checkedState === "checked"}
			className={classes(restProps.className, styles.row)}
		>
			{/* The chevron reports the state the row's own click toggles, so it is a
			    mark rather than a control — the checkbox is what takes clicks here. */}
			<div className={styles.leading}>
				<Icon
					size={14}
					className={styles.leadingMark}
					name={isCollapsed ? "chevron-right" : "chevron-down"}
				/>
				<RowCheckbox
					disabled={!isDefaultMode || !canCheck}
					aria-label={`Check directory ${path}`}
					checked={checkedState === "checked"}
					indeterminate={checkedState === "indeterminate"}
					className={styles.leadingCheckbox}
					nativeButton
					onCheckedChange={(checked) => {
						checkDirectory({ path, checked });
					}}
				/>
			</div>

			<RowLabelContainer>
				<RowLabel singleLine>{name}</RowLabel>
			</RowLabelContainer>

			{/* Collapsed, the count is the only sign of what the row is holding. */}
			{isCollapsed && (
				<span className={classes(styles.fileCount, rowStyles.fadedText, "text-13")}>
					{fileCount}
				</span>
			)}
		</Row>
	);
};
