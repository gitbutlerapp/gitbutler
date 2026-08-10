import { FolderIcon } from "#ui/components/FolderIcon.tsx";
import { classes } from "#ui/components/classes.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { projectSlice } from "#ui/projects/state.ts";
import type { SelectionScope } from "#ui/selection-scopes.ts";
import { useAppSelector } from "#ui/store.ts";
import { Tooltip } from "@base-ui/react";
import type { ComponentProps, FC } from "react";
import styles from "./FilesTree.module.css";
import rowStyles from "./Row.module.css";
import { Row, RowCheckbox, RowLabel, RowLabelContainer } from "./Row.tsx";
import { TreeSteps, TreeStepsToggle } from "./TreeSteps.tsx";

/** Whether every file below a directory is checked, some of them, or none. */
export type DirectoryCheckedState = "checked" | "indeterminate" | "unchecked";

export const DirectoryRow: FC<
	{
		projectId: string;
		path: string;
		/** The trailing path segments this row stands for, e.g. `src/lib`. */
		name: string;
		fileCount: number;
		depth: number;
		isCollapsed: boolean;
		onToggleCollapsed: () => void;
		canCheck: boolean;
		checkedState: DirectoryCheckedState;
		checkDirectory: (evt: { path: string; checked: boolean }) => void;
		selectionScope: SelectionScope;
	} & ComponentProps<typeof Row>
> = ({
	projectId,
	path,
	name,
	fileCount,
	depth,
	isCollapsed,
	onToggleCollapsed,
	canCheck,
	checkedState,
	checkDirectory,
	selectionScope,
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
			<TreeSteps depth={depth}>
				<Tooltip.Root disableHoverablePopup>
					<Tooltip.Trigger
						aria-label={`${isCollapsed ? "Expand" : "Collapse"} directory ${path}`}
						onClick={onToggleCollapsed}
						render={<TreeStepsToggle isCollapsed={isCollapsed} />}
					/>
					<Tooltip.Portal>
						<Tooltip.Positioner sideOffset={4}>
							<Tooltip.Popup
								render={
									<TooltipPopup
										kbd={changesFileHotkeys.toggleFoldDirectory.hotkey}
										kbdScope={selectionScope}
									/>
								}
							>
								{isCollapsed ? "Expand directory" : "Collapse directory"}
							</Tooltip.Popup>
						</Tooltip.Positioner>
					</Tooltip.Portal>
				</Tooltip.Root>
			</TreeSteps>

			{/* The folder stands where a file's type icon stands, and gives way to the
			    checkbox on the same terms. */}
			<div className={styles.leading}>
				<FolderIcon className={styles.leadingMark} />
				<RowCheckbox
					disabled={!isDefaultMode || !canCheck}
					aria-label={`Check directory ${path}`}
					checked={checkedState === "checked"}
					indeterminate={checkedState === "indeterminate"}
					className={styles.leadingCheckbox}
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
				<span className={classes(styles.fileCount, rowStyles.fadedText, "text-11")}>
					{fileCount}
				</span>
			)}
		</Row>
	);
};
