import rowStyles from "./Row.module.css";
import {
	changesInWorktreeQueryOptions,
	getGUISettingsQueryOptions,
	headInfoQueryOptions,
	listEditorsQueryOptions,
} from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { uncommittedChangesFileParent, fileOperand, FileParent } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { mergeProps, useRender } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { ComponentProps, FC, useRef } from "react";
import styles from "./FilesTree.module.css";
import { Row, RowLabel, RowLabelContainer } from "./Row.tsx";
import { TransferOperationSource } from "#ui/routes/project/$id/workspace/TransferOperationSource.tsx";
import { focusSelectionScope, useNavigationIndexHotkeys } from "#ui/selection-scopes.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { FileRow } from "./FileRow.tsx";
import type { FileRowItem } from "./file-row.ts";
import { useOpenInEditor } from "#ui/api/mutations.ts";

const useFilesTreeHotkeys = ({
	navigationIndex,
	onFileSelection,
	projectId,
	ref,
	fileParent,
	selection,
}: {
	navigationIndex: NavigationIndex<string>;
	onFileSelection: (selection: string) => void;
	projectId: string;
	ref: React.RefObject<HTMLElement | null>;
	fileParent: FileParent;
	selection: string | null;
}) => {
	const outlineMode = useAppSelector((state) =>
		projectSlice.selectors.selectOutlineModeState(state, projectId),
	);
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...getGUISettingsQueryOptions(),
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});
	const openInEditor = useOpenInEditor();

	const dispatch = useAppDispatch();

	const selectedChangesFile = fileParent._tag === "UncommittedChanges" ? selection : null;

	const absorbSelectedFile = () => {
		if (selectedChangesFile === null) return;

		const change = worktreeChanges?.changes.find((change) => change.path === selectedChangesFile);
		if (!change) return;

		dispatch(
			projectSlice.actions.enterAbsorbMode({
				projectId,
				source: fileOperand({ parent: uncommittedChangesFileParent, path: selectedChangesFile }),
				sourceTarget: {
					type: "treeChanges",
					subject: {
						changes: [change],
						assignedStackId: null,
					},
				},
			}),
		);
		focusSelectionScope("outline");
	};

	useHotkeys([
		{
			hotkey: changesFileHotkeys.absorb.hotkey,
			callback: absorbSelectedFile,
			options: {
				conflictBehavior: "allow",
				enabled: selectedChangesFile !== null && outlineMode._tag === "Default",
				target: ref,
				meta: changesFileHotkeys.absorb.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.openInEditor.hotkey,
			callback: () => {
				if (!preferredEditor || selectedChangesFile === null) return;

				openInEditor.mutate({
					projectId,
					editorId: preferredEditor.id,
					path: selectedChangesFile,
					lineNr: null,
				});
			},
			options: {
				conflictBehavior: "allow",
				enabled: preferredEditor && selectedChangesFile !== null,
				target: ref,
				meta: changesFileHotkeys.openInEditor.meta,
			},
		},
	]);

	useNavigationIndexHotkeys({
		navigationIndex,
		projectId,
		group: "File",
		select: onFileSelection,
		selection,
		ref,
		getKey: (path) => path,
		operationSourceForItem: (path) => fileOperand({ parent: fileParent, path }),
	});
};

export const FilesTree: FC<
	{
		projectId: string;
		items: Array<FileRowItem>;
		selection: string | null;
		onFileSelection: (selection: string) => void;
		navigationIndex: NavigationIndex<string>;
		fileParent: FileParent;
	} & ComponentProps<"div">
> = ({
	items,
	selection,
	onFileSelection,
	projectId,
	navigationIndex,
	fileParent,
	ref: refProp,
	...props
}) => {
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});

	const ref = useRef<HTMLDivElement>(null);

	useFilesTreeHotkeys({
		navigationIndex,
		onFileSelection,
		projectId,
		ref,
		fileParent,
		selection,
	});

	return (
		<div
			{...props}
			tabIndex={0}
			role="tree"
			aria-activedescendant={selection !== null ? treeItemId(selection) : undefined}
			className={classes(props.className, styles.tree)}
			ref={useMergedRefs(refProp, ref)}
		>
			<div className={styles.section}>
				{items.length === 0 ? (
					<Row interactive={false}>
						<RowLabelContainer>
							<RowLabel className={rowStyles.fadedText}>No changes.</RowLabel>
						</RowLabelContainer>
					</Row>
				) : (
					// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics.
					<div role="group">
						{items.map((item) => (
							<TreeItem
								key={item.path}
								isSelected={selection !== null && selection === item.path}
								aria-label={
									item._tag === "Change"
										? `${item.change.status.type} ${item.change.path}`
										: `Conflict ${item.path}`
								}
								path={item.path}
								render={
									<TransferOperationSource
										projectId={projectId}
										source={fileOperand({ parent: fileParent, path: item.path })}
										outline="outside"
										render={
											<FileRow
												item={item}
												inert={!navigationIndexIncludes(navigationIndex, item.path, (path) => path)}
												isSelected={selection !== null && selection === item.path}
												onSelect={() => onFileSelection(item.path)}
												projectId={projectId}
												fileParent={fileParent}
												branchNameByCommitId={(cid) =>
													headInfoIndex?.commitContextById(cid)?.segment.refName?.displayName
												}
											/>
										}
									/>
								}
							/>
						))}
					</div>
				)}
			</div>
		</div>
	);
};

const treeItemId = (path: string): string => `files-treeitem-${encodeURIComponent(path)}`;

const TreeItem: FC<
	{
		path: string;
		isSelected: boolean;
	} & useRender.ComponentProps<"div">
> = ({ path, isSelected, render, ...props }) =>
	useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(path),
			role: "treeitem",
			"aria-selected": isSelected,
		}),
	});
