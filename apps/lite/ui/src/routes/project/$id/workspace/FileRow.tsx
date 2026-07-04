import { FileIcon } from "#ui/components/FileIcon.tsx";
import rowStyles from "./Row.module.css";
import {
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { type FileOperand, type FileParent } from "#ui/operands.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { classes } from "#ui/components/classes.ts";
import { Match } from "effect";
import {
	ComponentProps,
	createContext,
	FC,
	PointerEvent as ReactPointerEvent,
	ReactNode,
	use,
	useRef,
	useState,
} from "react";
import { createPortal } from "react-dom";
import styles from "./FileRow.module.css";
import { Row, RowCheckbox, RowLabel, RowLabelContainer, RowToolbar } from "./Row.tsx";
import { getRowButtonClassName } from "./Row-utils.ts";
import { DependencyIndicator } from "#ui/routes/project/$id/workspace/DependencyIndicator.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import type { FileRowItem } from "./file-row.ts";

type FileRowTooltipState = {
	content: ReactNode;
	left: number;
	top: number;
};

type FileRowTooltipContextValue = {
	hide: () => void;
	show: (anchor: HTMLElement, content: ReactNode) => void;
};

const FileRowTooltipContext = createContext<FileRowTooltipContextValue | null>(null);

export const FileRowTooltipProvider: FC<{ children: ReactNode }> = ({ children }) => {
	const [tooltip, setTooltip] = useState<FileRowTooltipState | null>(null);
	const tooltipApi = useRef<FileRowTooltipContextValue>(null);
	tooltipApi.current ??= {
		hide: () => setTooltip(null),
		show: (anchor, content) => {
			const rect = anchor.getBoundingClientRect();
			setTooltip({
				content,
				left: Math.round(rect.left),
				top: Math.round(rect.bottom + 4),
			});
		},
	};

	return (
		<FileRowTooltipContext value={tooltipApi.current}>
			{children}
			{tooltip &&
				createPortal(
					<div
						className={styles.tooltipPositioner}
						style={{
							left: tooltip.left,
							top: tooltip.top,
						}}
					>
						<TooltipPopup role="tooltip">{tooltip.content}</TooltipPopup>
					</div>,
					document.body,
				)}
		</FileRowTooltipContext>
	);
};

export const FileRow: FC<
	{
		item: FileRowItem;
		projectId: string;
		fileParent: FileParent;
		getFileMenuItems: (input: {
			operand: FileOperand;
			path: string;
			change?: Extract<FileRowItem, { _tag: "Change" }>["change"];
		}) => Array<NativeMenuItem>;
		hasCheckedCommits: boolean;
		isDefaultMode: boolean;
		branchNameByCommitId?: (commitId: string) => string | undefined;
	} & Omit<ComponentProps<typeof Row>, "projectId">
> = ({
	item,
	projectId,
	fileParent,
	getFileMenuItems,
	hasCheckedCommits,
	isDefaultMode,
	branchNameByCommitId,
	id,
	...restProps
}) => {
	const relativePath = item._tag === "Change" ? item.change.path : item.path;
	const getMenuItems = () =>
		getFileMenuItems({
			operand: { parent: fileParent, path: relativePath },
			path: relativePath,
			change: item._tag === "Change" ? item.change : undefined,
		});

	const lastSepIdx = relativePath.lastIndexOf("/");
	const directoryPath = lastSepIdx !== -1 ? relativePath.slice(0, lastSepIdx) : null;
	const fileName = lastSepIdx !== -1 ? relativePath.slice(lastSepIdx + 1) : relativePath;
	const tooltip = use(FileRowTooltipContext);
	const showTooltip = (anchor: HTMLElement, content: ReactNode) => tooltip?.show(anchor, content);
	const hideTooltip = () => tooltip?.hide();
	const showRowTooltip = (anchor: HTMLElement) => showTooltip(anchor, relativePath);
	const restoreRowTooltip = (event: ReactPointerEvent<HTMLElement>) => {
		const relatedTarget = event.relatedTarget instanceof Node ? event.relatedTarget : null;
		const row = event.currentTarget.closest('[role="treeitem"]');
		if (row instanceof HTMLElement && relatedTarget && row.contains(relatedTarget))
			showRowTooltip(row);
		else hideTooltip();
	};

	return (
		<Row
			{...restProps}
			id={id}
			className={classes(restProps.className, styles.row)}
			onBlurCapture={(event) => {
				restProps.onBlurCapture?.(event);
				hideTooltip();
			}}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, getMenuItems());
			}}
			onFocusCapture={(event) => {
				restProps.onFocusCapture?.(event);
				showRowTooltip(event.currentTarget);
			}}
			onPointerEnter={(event) => {
				restProps.onPointerEnter?.(event);
				showRowTooltip(event.currentTarget);
			}}
			onPointerLeave={(event) => {
				restProps.onPointerLeave?.(event);
				hideTooltip();
			}}
		>
			<div className={styles.iconWithCheckbox}>
				<FileIcon fileName={fileName} className={styles.icon} />
				<RowCheckbox
					disabled={hasCheckedCommits || !isDefaultMode}
					aria-label={`Check file ${relativePath}`}
					className={styles.checkbox}
					nativeButton
					onBlur={hideTooltip}
					onFocus={(event) => showTooltip(event.currentTarget, "Check file")}
					onPointerEnter={(event) => showTooltip(event.currentTarget, "Check file")}
					onPointerLeave={restoreRowTooltip}
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

			{isDefaultMode && (
				<RowToolbar aria-label="File actions">
					<button
						type="button"
						aria-label="File menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, getMenuItems());
						}}
						className={getRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</button>
				</RowToolbar>
			)}

			{isDefaultMode &&
				item._tag === "Change" &&
				fileParent._tag === "UncommittedChanges" &&
				item.dependencyCommitIds && (
					<RowToolbar forceVisible aria-label="File dependencies">
						<DependencyIndicator
							projectId={projectId}
							commitIds={item.dependencyCommitIds}
							branchNameByCommitId={branchNameByCommitId}
							className={getRowButtonClassName({ iconOnly: true })}
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
					onBlur={hideTooltip}
					onFocus={(event) => showTooltip(event.currentTarget, item.change.status.type)}
					onPointerEnter={(event) => showTooltip(event.currentTarget, item.change.status.type)}
					onPointerLeave={restoreRowTooltip}
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
