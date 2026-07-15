import { FileIcon } from "#ui/components/FileIcon.tsx";
import { CheckedCommitIdsContext } from "#ui/CheckedCommitIdsContext.ts";
import rowStyles from "./Row.module.css";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { FileParent } from "#ui/operands.ts";
import { OutlineModeContext } from "#ui/WorkspaceContext.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { classes } from "#ui/components/classes.ts";
import { Match } from "effect";
import { ComponentProps, FC, use } from "react";
import styles from "./FileRow.module.css";
import { Row, RowCheckbox, RowLabel, RowLabelContainer, RowToolbar } from "./Row.tsx";
import { getRowButtonClassName } from "./Row-utils.ts";
import { DependencyIndicator } from "#ui/routes/project/$id/workspace/DependencyIndicator.tsx";
import { useFileMenuItems } from "#ui/routes/project/$id/workspace/useFileMenuItems.ts";
import type { FileRowItem } from "./file-row.ts";

export const FileRow: FC<
	{
		item: FileRowItem;
		projectId: string;
		fileParent: FileParent;
		branchNameByCommitId: (commitId: string) => string | undefined;
	} & Omit<ComponentProps<typeof Row>, "projectId">
> = ({ item, projectId, fileParent, branchNameByCommitId, id, ...restProps }) => {
	const { checkedCommitIds } = use(CheckedCommitIdsContext);
	const relativePath = item._tag === "Change" ? item.change.path : item.path;

	const { outlineMode } = use(OutlineModeContext);
	const menuItems = useFileMenuItems({
		projectId,
		operand: { parent: fileParent, path: relativePath },
		path: relativePath,
		change: item._tag === "Change" ? item.change : undefined,
	});

	const hasCheckedCommits = checkedCommitIds.size > 0;

	const lastSepIdx = relativePath.lastIndexOf("/");
	const directoryPath = lastSepIdx !== -1 ? relativePath.slice(0, lastSepIdx) : null;
	const fileName = lastSepIdx !== -1 ? relativePath.slice(lastSepIdx + 1) : relativePath;

	return (
		<Row
			{...restProps}
			id={id}
			className={classes(restProps.className, styles.row)}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<div className={styles.iconWithCheckbox}>
				<FileIcon fileName={fileName} className={styles.icon} />
				<RowCheckbox
					disabled={hasCheckedCommits || outlineMode._tag !== "Default"}
					aria-label={`Check file ${relativePath}`}
					className={styles.checkbox}
					nativeButton
				/>
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
				<RowToolbar aria-label="File actions" role="toolbar">
					<button
						aria-label="File menu"
						type="button"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</button>
				</RowToolbar>
			)}

			{outlineMode._tag === "Default" &&
				item._tag === "Change" &&
				fileParent._tag === "UncommittedChanges" &&
				item.dependencyCommitIds.length > 0 && (
					<RowToolbar aria-label="File actions" forceVisible role="toolbar">
						<DependencyIndicator
							projectId={projectId}
							commitIds={item.dependencyCommitIds}
							branchNameByCommitId={branchNameByCommitId}
							className={getRowButtonClassName({ iconOnly: true })}
							type="button"
						>
							<Icon name="link" />
						</DependencyIndicator>
					</RowToolbar>
				)}

			{item._tag === "Change" && (
				<span
					className={styles.statusBadge}
					aria-label={item.change.status.type}
					data-status-type={item.change.status.type}
				>
					{Match.value(item.change.status).pipe(
						Match.when({ type: "Addition" }, () => "A"),
						Match.when({ type: "Deletion" }, () => "D"),
						Match.when({ type: "Modification" }, () => "M"),
						Match.when({ type: "Rename" }, () => "R"),
						Match.exhaustive,
					)}
				</span>
			)}
		</Row>
	);
};
