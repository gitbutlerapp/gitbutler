import rowStyles from "./Row.module.css";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { FileParent } from "#ui/operands.ts";
import {
	selectProjectHasCheckedCommits,
	selectProjectOutlineModeState,
} from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { Checkbox } from "#ui/components/Checkbox.tsx";
import { classes } from "#ui/components/classes.ts";
import { Tooltip } from "@base-ui/react";
import { Toolbar } from "@base-ui/react/toolbar";
import { Match } from "effect";
import { ComponentProps, FC } from "react";
import styles from "./FileRow.module.css";
import { Row, RowLabel, RowLabelContainer, RowToolbar } from "./Row.tsx";
import { getRowButtonClassName } from "./Row-utils.ts";
import { DependencyIndicator } from "#ui/routes/project/$id/workspace/DependencyIndicator.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { useFileMenuItems } from "#ui/routes/project/$id/workspace/useFileMenuItems.ts";
import type { FileRowItem } from "./file-row.ts";

export const FileRow: FC<
	{
		item: FileRowItem;
		projectId: string;
		fileParent: FileParent;
		branchNameByCommitId?: (commitId: string) => string | undefined;
	} & Omit<ComponentProps<typeof Row>, "projectId">
> = ({ item, projectId, fileParent, branchNameByCommitId, id, ...restProps }) => {
	const relativePath = item._tag === "Change" ? item.change.path : item.path;

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const menuItems = useFileMenuItems({
		projectId,
		operand: { parent: fileParent, path: relativePath },
		path: relativePath,
		change: item._tag === "Change" ? item.change : undefined,
	});

	const hasCheckedCommits = useAppSelector((state) =>
		selectProjectHasCheckedCommits(state, projectId),
	);

	const lastSepIdx = relativePath.lastIndexOf("/");
	const directoryPath = lastSepIdx !== -1 ? relativePath.slice(0, lastSepIdx) : null;
	const fileName = lastSepIdx !== -1 ? relativePath.slice(lastSepIdx + 1) : relativePath;

	return (
		<Tooltip.Root disableHoverablePopup>
			<Tooltip.Trigger
				// We pass the ID here instead of including it with the other props as a
				// workaround for Base UI issue:
				// https://github.com/mui/base-ui/issues/5108
				id={id}
				render={
					<Row
						{...restProps}
						className={classes(restProps.className, styles.fileRow)}
						onContextMenu={(event) => {
							void showNativeContextMenu(event, menuItems);
						}}
					/>
				}
			>
				<div className={styles.fileIconWithCheckbox}>
					<Icon name="file" />
					<Tooltip.Root
						// This gets in the way when the user tries to move their hover to a
						// sibling row.
						disableHoverablePopup
					>
						<Checkbox
							disabled={hasCheckedCommits || outlineMode._tag !== "Default"}
							aria-label={`Check file ${relativePath}`}
							className={styles.fileCheckbox}
							nativeButton
							render={<Tooltip.Trigger />}
						/>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup render={<TooltipPopup />}>Check file</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				</div>

				<RowLabelContainer>
					{item._tag === "Conflict" && "⚠️"}
					<RowLabel singleLine>
						{fileName}
						{directoryPath !== null && (
							<span className={classes(styles.pathInit, rowStyles.fadedText)}>{directoryPath}</span>
						)}
					</RowLabel>
				</RowLabelContainer>

				{outlineMode._tag === "Default" && (
					<Toolbar.Root aria-label="File actions" render={<RowToolbar />}>
						<Toolbar.Button
							aria-label="File menu"
							onClick={(event) => {
								void showNativeMenuFromTrigger(event.currentTarget, menuItems);
							}}
							className={getRowButtonClassName({ iconOnly: true })}
						>
							<Icon name="kebab" />
						</Toolbar.Button>
					</Toolbar.Root>
				)}

				{outlineMode._tag === "Default" &&
					item._tag === "Change" &&
					fileParent._tag === "UncommittedChanges" &&
					item.dependencyCommitIds && (
						<Toolbar.Root aria-label="File actions" render={<RowToolbar forceVisible />}>
							<Toolbar.Button
								render={
									<DependencyIndicator
										projectId={projectId}
										commitIds={item.dependencyCommitIds}
										branchNameByCommitId={branchNameByCommitId}
										className={getRowButtonClassName({ iconOnly: true })}
									/>
								}
							>
								<Icon name="link" />
							</Toolbar.Button>
						</Toolbar.Root>
					)}

				{item._tag === "Change" && (
					<Tooltip.Root disableHoverablePopup>
						<Tooltip.Trigger
							className={styles.fileStatusBadge}
							aria-label={item.change.status.type}
							data-status-type={item.change.status.type}
							// By default it's a button, but we don't want this to be
							// interactive.
							render={<span />}
						>
							{Match.value(item.change.status).pipe(
								Match.when({ type: "Addition" }, () => "A"),
								Match.when({ type: "Deletion" }, () => "D"),
								Match.when({ type: "Modification" }, () => "M"),
								Match.when({ type: "Rename" }, () => "R"),
								Match.exhaustive,
							)}
						</Tooltip.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup render={<TooltipPopup />}>{item.change.status.type}</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				)}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{relativePath}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
