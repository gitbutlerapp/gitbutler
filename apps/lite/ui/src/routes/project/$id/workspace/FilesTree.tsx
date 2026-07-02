import rowStyles from "./Row.module.css";
import { changesInWorktreeQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { uncommittedChangesFileParent, fileOperand, FileParent } from "#ui/operands.ts";
import {
	projectActions,
	selectProjectOutlineModeState,
	selectProjectSelectionFiles,
} from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { mergeProps, useRender } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { identity } from "effect";
import { ComponentProps, createContext, FC, use, useRef } from "react";
import styles from "./FilesTree.module.css";
import { Row, RowLabel, RowLabelContainer } from "./Row.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import {
	focusSelectionScope,
	resolveNavigationIndexSelection,
	useFilesSelection,
	useNavigationIndexHotkeys,
} from "#ui/selection-scopes.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { assert } from "#ui/assert.ts";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { FileRow } from "./FileRow.tsx";
import type { FileRowItem } from "./file-row.ts";

const NavigationIndexContext = createContext<NavigationIndex<string> | null>(null);

const useFilesTreeHotkeys = ({
	navigationIndex,
	onFileSelection,
	projectId,
	ref,
	fileParent,
}: {
	navigationIndex: NavigationIndex<string>;
	onFileSelection: (selection: string) => void;
	projectId: string;
	ref: React.RefObject<HTMLElement | null>;
	fileParent: FileParent;
}) => {
	const selection = useFilesSelection(projectId, navigationIndex);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const dispatch = useAppDispatch();

	const selectedChangesFile = fileParent._tag === "UncommittedChanges" ? selection : null;

	const absorbSelectedFile = () => {
		if (selectedChangesFile === null) return;

		const change = worktreeChanges?.changes.find((change) => change.path === selectedChangesFile);
		if (!change) return;

		dispatch(
			projectActions.enterAbsorbMode({
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
	]);

	useNavigationIndexHotkeys({
		navigationIndex,
		projectId,
		group: "File",
		select: onFileSelection,
		selection,
		ref,
		getKey: identity,
		operationSourceForItem: (path) => fileOperand({ parent: fileParent, path }),
	});
};

export const FilesTree: FC<
	{
		projectId: string;
		items: Array<FileRowItem>;
		onFileSelection: (selection: string) => void;
		navigationIndex: NavigationIndex<string>;
		fileParent: FileParent;
	} & ComponentProps<"div">
> = ({
	items,
	onFileSelection,
	projectId,
	navigationIndex,
	fileParent,
	ref: refProp,
	...props
}) => {
	const selection = useFilesSelection(projectId, navigationIndex);
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
	});

	return (
		<NavigationIndexContext value={navigationIndex}>
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
									projectId={projectId}
									path={item.path}
									aria-label={
										item._tag === "Change"
											? `${item.change.status.type} ${item.change.path}`
											: `Conflict ${item.path}`
									}
									render={
										<OperationSourceC
											projectId={projectId}
											source={fileOperand({ parent: fileParent, path: item.path })}
											render={
												<FileRow
													item={item}
													inert={!navigationIndexIncludes(navigationIndex, item.path, identity)}
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
		</NavigationIndexContext>
	);
};

const useIsSelected = ({ projectId, path }: { projectId: string; path: string }): boolean => {
	const navigationIndex = assert(use(NavigationIndexContext));
	return useAppSelector((state) => {
		const selectionState = selectProjectSelectionFiles(state, projectId);
		const selection = resolveNavigationIndexSelection(navigationIndex, selectionState, identity);

		return selection !== null && selection === path;
	});
};

const treeItemId = (path: string): string => `files-treeitem-${encodeURIComponent(path)}`;

const TreeItem: FC<
	{
		projectId: string;
		path: string;
	} & useRender.ComponentProps<"div">
> = ({ projectId, path, render, ...props }) => {
	const isSelected = useIsSelected({ projectId, path });

	return useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(path),
			role: "treeitem",
			"aria-selected": isSelected,
		}),
	});
};
