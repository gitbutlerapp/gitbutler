import rowStyles from "./Row.module.css";
import {
	changesInWorktreeQueryOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	listEditorsQueryOptions,
} from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import {
	uncommittedChangesFileParent,
	fileOperand,
	operandEquals,
	operandIdentityKey,
	type FileParent,
} from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { mergeProps, useRender } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { type ComponentProps, type FC, useRef } from "react";
import styles from "./FilesTree.module.css";
import { Row, RowLabel, RowLabelContainer } from "./Row.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { focusSelectionScope, useNavigationIndexHotkeys } from "#ui/selection-scopes.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { FileRow } from "./FileRow.tsx";
import type { FileRowItem } from "./file-row.ts";
import { useOpenInProgram } from "#ui/api/mutations.ts";
import { checkedRange, navigationIndexRange } from "#ui/checking.ts";

const useFilesTreeHotkeys = ({
	checkFile,
	navigationIndex,
	onFileSelection,
	projectId,
	ref,
	fileParent,
	selection,
}: {
	checkFile: (evt: { path: string; shiftKey: boolean }) => void;
	navigationIndex: NavigationIndex<string>;
	onFileSelection: (selection: string) => void;
	projectId: string;
	ref: React.RefObject<HTMLElement | null>;
	fileParent: FileParent;
	selection: string | null;
}) => {
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});
	const { mutate: openInProgram } = useOpenInProgram();

	const store = useAppStore();
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

	const toggleSelectedFileChecked = (event: KeyboardEvent) => {
		if (selection === null) return;
		// Leave activation of a directly focused checkbox to the checkbox itself.
		if (event.target !== ref.current) return;

		event.preventDefault();
		event.stopPropagation();
		checkFile({ path: selection, shiftKey: event.shiftKey });
	};

	useHotkeys([
		{
			hotkey: changesFileHotkeys.absorb.hotkey,
			callback: absorbSelectedFile,
			options: {
				conflictBehavior: "allow",
				enabled: selectedChangesFile !== null && isDefaultMode,
				target: ref,
				meta: changesFileHotkeys.absorb.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.checkFile.hotkey,
			callback: toggleSelectedFileChecked,
			options: {
				conflictBehavior: "allow",
				enabled: selection !== null && isDefaultMode,
				preventDefault: false,
				stopPropagation: false,
				target: ref,
				meta: changesFileHotkeys.checkFile.meta,
			},
		},
		{
			hotkey: "Shift+Space",
			callback: toggleSelectedFileChecked,
			options: {
				conflictBehavior: "allow",
				enabled: selection !== null && isDefaultMode,
				preventDefault: false,
				stopPropagation: false,
				target: ref,
			},
		},
		{
			hotkey: changesFileHotkeys.openInEditor.hotkey,
			callback: () => {
				if (!preferredEditor || selectedChangesFile === null) return;

				openInProgram({
					projectId,
					programId: preferredEditor.id,
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
		operationSourcesForItem: (path) => {
			const operand = fileOperand({ parent: fileParent, path });
			const checkedOperands = projectSlice.selectors.selectCheckedOperands(
				store.getState(),
				projectId,
			);
			return checkedOperands.length > 0 ? checkedOperands : [operand];
		},
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
		emptyLabel?: string;
	} & ComponentProps<"div">
> = ({
	items,
	selection,
	onFileSelection,
	projectId,
	navigationIndex,
	fileParent,
	emptyLabel = "No changes.",
	ref: refProp,
	...props
}) => {
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const checkedOperandKeys = useAppSelector((state) =>
		projectSlice.selectors.selectCheckedOperandKeys(state, projectId),
	);
	const store = useAppStore();
	const dispatch = useAppDispatch();

	const ref = useRef<HTMLDivElement>(null);

	const fileCheckRangeAnchor = useRef<string>(null);
	const fileCheckRangeEnd = useRef<string>(null);

	const rangeResolver = navigationIndexRange<string, string>({
		navigationIndex,
		getKey: (path) => path,
		filterMap: (path) => path,
	});
	const getCheckedRange = checkedRange(rangeResolver);

	const checkFile = ({ path, shiftKey }: { path: string; shiftKey: boolean }): void => {
		const checkedOperands = projectSlice.selectors.selectCheckedOperands(
			store.getState(),
			projectId,
		);
		const checkedFilePaths = new Set(
			checkedOperands.flatMap((operand) =>
				operand._tag === "File" && operandEquals(operand.parent, fileParent) ? operand.path : [],
			),
		);
		const nextFileRange = getCheckedRange({
			checked: checkedFilePaths,
			rangeAnchor: fileCheckRangeAnchor.current,
			rangeEnd: fileCheckRangeEnd.current,
		})({
			item: path,
			shiftKey,
		});

		fileCheckRangeAnchor.current = nextFileRange.rangeAnchor;
		fileCheckRangeEnd.current = nextFileRange.rangeEnd;

		const checkedFiles = nextFileRange.checked.difference(checkedFilePaths);
		const uncheckedFiles = checkedFilePaths.difference(nextFileRange.checked);
		dispatch(
			projectSlice.actions.checkOperands({
				projectId,
				operands: Array.from(checkedFiles, (path) => fileOperand({ parent: fileParent, path })),
				checked: true,
			}),
		);
		dispatch(
			projectSlice.actions.checkOperands({
				projectId,
				operands: Array.from(uncheckedFiles, (path) => fileOperand({ parent: fileParent, path })),
				checked: false,
			}),
		);
	};

	useFilesTreeHotkeys({
		checkFile,
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
			{items.length === 0 ? (
				<Row interactive={false}>
					<RowLabelContainer>
						<RowLabel className={rowStyles.fadedText}>{emptyLabel}</RowLabel>
					</RowLabelContainer>
				</Row>
			) : (
				// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics.
				<div role="group">
					{items.map((item) => {
						const operand = fileOperand({ parent: fileParent, path: item.path });
						return (
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
									<OperationSourceC
										projectId={projectId}
										source={operand}
										outline="outside"
										render={
											<FileRow
												item={item}
												inert={!navigationIndexIncludes(navigationIndex, item.path, (path) => path)}
												isSelected={selection !== null && selection === item.path}
												isChecked={checkedOperandKeys.has(operandIdentityKey(operand))}
												onSelect={() => onFileSelection(item.path)}
												checkFile={checkFile}
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
						);
					})}
				</div>
			)}
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
