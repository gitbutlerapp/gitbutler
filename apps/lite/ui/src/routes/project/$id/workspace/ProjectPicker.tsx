import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { FolderIcon } from "#ui/components/FolderIcon.tsx";
import { Popup, PopupItem, PopupSearch, PopupSection } from "#ui/components/Popup.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { useAddLocalRepository } from "#ui/components/useAddLocalRepository.ts";
import { globalHotkeys } from "#ui/hotkeys.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { listProjectsQueryOptions, repoInfoQueryOptions } from "#ui/api/queries.ts";
import { getRangeExtractorWithIndices } from "#ui/virtual.ts";
import {
	readProjectsOpenedAt,
	readProjectsRepoMarks,
	writeLastOpenedProject,
	writeProjectRepoMarks,
	type ProjectRepoMarks,
} from "#ui/project.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Button, Combobox, Tooltip } from "@base-ui/react";
import type { IconName } from "#ui/components/iconNames.ts";
import type { ProjectForFrontend } from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { type Range, useVirtualizer } from "@tanstack/react-virtual";
import {
	type FC,
	type RefObject,
	useCallback,
	useDeferredValue,
	useEffect,
	useId,
	useImperativeHandle,
	useMemo,
	useRef,
	useState,
} from "react";
// The trigger is the header's project name, and the header's container query hides its label
// when the sidebar narrows — so the button keeps wearing the header's classes.
import headerStyles from "./SidebarHeader.module.css";
import styles from "./ProjectPicker.module.css";

type ProjectGroup = { value: string; items: Array<ProjectForFrontend> };

type VirtualProject = {
	groupIndex: number;
	isFirstInGroup: boolean;
	project: ProjectForFrontend;
};

type ProjectVirtualizer = ReturnType<typeof useVirtualizer<HTMLDivElement, Element>>;

/** How many projects lead the list before the rest are folded into "Older". */
const recentCount = 5;

/**
 * Private outranks fork, and a row has one glyph: who can see a repository is the sharper thing to
 * know at a glance than where it came from. A project whose forge has not been asked yet — or has
 * none to ask — is just a folder.
 */
const projectIcon = (marks: ProjectRepoMarks | undefined): IconName =>
	marks?.private === true ? "folder-lock" : marks?.fork === true ? "folder-fork" : "folder";

/**
 * The ones in use, most recently opened first, then everything else by name. Before anything has
 * been opened there is no recency to speak of, so the list stays one plain group rather than
 * heading an empty half.
 */
const groupProjects = (
	projects: Array<ProjectForFrontend>,
	openedAt: Record<string, number>,
): Array<ProjectGroup> => {
	const byName = (a: ProjectForFrontend, b: ProjectForFrontend) => a.title.localeCompare(b.title);
	const opened = projects
		.filter((project) => openedAt[project.id] !== undefined)
		.toSorted((a, b) => (openedAt[b.id] ?? 0) - (openedAt[a.id] ?? 0));

	if (opened.length === 0) return [{ value: "Projects", items: projects.toSorted(byName) }];

	const recent = opened.slice(0, recentCount);
	const recentIds = new Set(recent.map((project) => project.id));
	const older = projects.filter((project) => !recentIds.has(project.id)).toSorted(byName);

	return [
		{ value: "Recent projects", items: recent },
		...(older.length > 0 ? [{ value: "Older", items: older }] : []),
	];
};

/** The two localStorage records the list orders and labels itself from, read together. */
const readRecords = () => ({
	openedAt: readProjectsOpenedAt(),
	marksById: readProjectsRepoMarks(),
});

const VirtualizedProjectList: FC<{
	currentProjectId: string;
	highlightedProjectIndex: number | null;
	marksById: Record<string, ProjectRepoMarks>;
	virtualizerRef: RefObject<ProjectVirtualizer | null>;
}> = ({ currentProjectId, highlightedProjectIndex, marksById, virtualizerRef }) => {
	const scrollElementRef = useRef<HTMLDivElement | null>(null);
	const groupDescriptionId = useId();
	const filteredGroups = Combobox.useFilteredItems<ProjectGroup>();

	// React Compiler leaves components using useVirtualizer uncompiled, hence manual memo.
	const virtualProjects = useMemo(
		() =>
			filteredGroups.flatMap((group, groupIndex) =>
				group.items.map(
					(project, projectIndex): VirtualProject => ({
						groupIndex,
						isFirstInGroup: projectIndex === 0,
						project,
					}),
				),
			),
		[filteredGroups],
	);

	const pinnedIndices = useMemo(() => {
		const indices = new Set<number>();
		let groupStartIndex = 0;

		for (const group of filteredGroups) {
			if (group.items.length === 0) continue;

			indices.add(groupStartIndex);
			groupStartIndex += group.items.length;
		}

		if (highlightedProjectIndex !== null) indices.add(highlightedProjectIndex);

		return Array.from(indices);
	}, [filteredGroups, highlightedProjectIndex]);

	const getVirtualRowKey = useCallback(
		(index: number) => virtualProjects[index]?.project.id ?? index,
		[virtualProjects],
	);

	const rangeExtractorWithPinnedRows = useCallback(
		(range: Range) => getRangeExtractorWithIndices(range, pinnedIndices),
		[pinnedIndices],
	);

	// oxlint-disable-next-line react-hooks-js/incompatible-library -- https://github.com/TanStack/virtual/issues/1119#issuecomment-4648268095
	const virtualizer = useVirtualizer({
		directDomUpdates: true,
		directDomUpdatesMode: "transform",
		count: virtualProjects.length,
		getScrollElement: () => scrollElementRef.current,
		estimateSize: (index) => {
			const row = virtualProjects[index];
			if (!row?.isFirstInGroup) return 28;
			return row.groupIndex > 0 ? 59 : 54;
		},
		getItemKey: getVirtualRowKey,
		rangeExtractor: rangeExtractorWithPinnedRows,
		paddingEnd: 4,
		scrollPaddingStart: 8,
		scrollPaddingEnd: 8,
	});
	useImperativeHandle(virtualizerRef, () => virtualizer);

	return (
		<Combobox.List ref={scrollElementRef} className={styles.list}>
			{virtualProjects.length > 0 && (
				<div ref={virtualizer.containerRef} role="presentation" className={styles.virtualContainer}>
					{virtualizer.getVirtualItems().map((virtualItem) => {
						const row = virtualProjects[virtualItem.index];
						if (row === undefined) return null;
						const group = filteredGroups[row.groupIndex];
						if (group === undefined) return null;

						return (
							<div
								key={virtualItem.key}
								ref={virtualizer.measureElement}
								data-index={virtualItem.index}
								role="presentation"
								className={styles.virtualProject}
							>
								{row.isFirstInGroup && (
									<div
										id={`${groupDescriptionId}-${row.groupIndex}`}
										aria-hidden="true"
										className={classes(
											"text-12",
											styles.groupLabel,
											row.groupIndex > 0 && styles.groupLabelDivided,
										)}
									>
										{group.value}
									</div>
								)}

								<PopupItem
									icon={projectIcon(marksById[row.project.id])}
									trailing={row.project.id === currentProjectId ? "tick" : undefined}
									className={styles.projectItem}
									render={
										<Combobox.Item
											index={virtualItem.index}
											value={row.project}
											aria-describedby={`${groupDescriptionId}-${row.groupIndex}`}
											aria-setsize={virtualProjects.length}
											aria-posinset={virtualItem.index + 1}
										/>
									}
								>
									{row.project.title}
								</PopupItem>
							</div>
						);
					})}
				</div>
			)}
		</Combobox.List>
	);
};

export const ProjectPicker: FC<{ project: ProjectForFrontend }> = (p) => {
	const navigate = useNavigate();
	const dispatch = useAppDispatch();
	const dialog = useAppSelector(interfaceSlice.selectors.selectDialogState);
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const { addLocalRepository, isPending: isAddingProject } = useAddLocalRepository();
	const [query, setQuery] = useState("");
	const [highlightedProjectIndex, setHighlightedProjectIndex] = useState<number | null>(null);
	const deferredQuery = useDeferredValue(query);
	const virtualizerRef = useRef<ProjectVirtualizer | null>(null);

	// What a repository is cannot be read off a clone, so it is asked of the forge for whichever
	// project is open and remembered. The list paints from what has been learned that way — a
	// project not yet opened since the record began simply shows the plain folder.
	const { data: repoInfo } = useQuery(repoInfoQueryOptions(p.project.id));
	useEffect(() => {
		if (repoInfo === undefined) return;
		writeProjectRepoMarks(p.project.id, { private: repoInfo.private, fork: repoInfo.fork });
	}, [p.project.id, repoInfo]);

	// Both records are read on the way into the picker rather than per render: they are written on
	// the way out of it, so what they order and label has to hold still while the list is up. The
	// reading then stays put once the picker closes — it is still on screen until its exit
	// animation finishes, and rereading there would reorder the list as it fades.
	const open = dialog._tag === "ProjectPicker";
	const [records, setRecords] = useState(readRecords);
	const [recordsAreFor, setRecordsAreFor] = useState(open);
	if (open !== recordsAreFor) {
		setRecordsAreFor(open);
		if (open) setRecords(readRecords());
	}

	const groups = useMemo(
		() => groupProjects(projects, records.openedAt),
		[projects, records.openedAt],
	);

	// The hotkey opens the picker from anywhere in the app, so what is open stays the dialog state's
	// to say; the trigger reports its own clicks back into it.
	const setOpen = (next: boolean) => {
		dispatch(
			next
				? interfaceSlice.actions.openDialog({ dialog: { _tag: "ProjectPicker" } })
				: interfaceSlice.actions.closeDialog(),
		);
	};

	const selectProject = (project: ProjectForFrontend | null) => {
		if (project === null) return;
		setOpen(false);
		void navigate({ to: "/project/$id/workspace", params: { id: project.id } });
		writeLastOpenedProject(project.id);
	};

	return (
		<Combobox.Root<ProjectForFrontend>
			items={groups}
			open={open}
			onOpenChange={setOpen}
			value={p.project}
			onValueChange={selectProject}
			inputValue={deferredQuery}
			onInputValueChange={setQuery}
			itemToStringLabel={(project) => project.title}
			itemToStringValue={(project) => project.id}
			isItemEqualToValue={(a, b) => a.id === b.id}
			autoHighlight
			virtualized
			onItemHighlighted={(project, { reason, index }) => {
				setHighlightedProjectIndex(project === undefined ? null : index);

				const virtualizer = virtualizerRef.current;
				if (project === undefined || !virtualizer) return;

				const isStart = index === 0;
				const isEnd = index === virtualizer.options.count - 1;
				if (reason === "none" || (reason === "keyboard" && (isStart || isEnd))) {
					queueMicrotask(() => {
						virtualizerRef.current?.scrollToIndex(index, { align: isEnd ? "start" : "end" });
					});
				}
			}}
		>
			<Tooltip.Root>
				<Combobox.Trigger
					className={classes(
						getButtonClassName({ variant: "ghost" }),
						"text-15",
						"text-bold",
						headerStyles.workspaceName,
					)}
					aria-label={`${globalHotkeys.selectProject.meta.name} (current: ${p.project.title})`}
					render={<Button render={<Tooltip.Trigger />} />}
				>
					<FolderIcon className={headerStyles.workspaceNameFolder} />
					<span className={headerStyles.workspaceNameLabel}>{p.project.title}</span>
				</Combobox.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup render={<TooltipPopup kbd={globalHotkeys.selectProject.hotkey} />}>
							{globalHotkeys.selectProject.meta.name}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>

			<Combobox.Portal>
				<Combobox.Positioner align="start" sideOffset={4}>
					<Popup anchored className={styles.popup} render={<Combobox.Popup />}>
						<PopupSearch
							placeholder="Search projects…"
							aria-label="Search projects"
							onClear={query === "" ? undefined : () => setQuery("")}
							render={<Combobox.Input value={query} />}
						/>
						<Combobox.Empty>
							<div className={classes("text-13", styles.empty)}>No projects found.</div>
						</Combobox.Empty>

						<VirtualizedProjectList
							currentProjectId={p.project.id}
							highlightedProjectIndex={highlightedProjectIndex}
							marksById={records.marksById}
							virtualizerRef={virtualizerRef}
						/>

						<PopupSection className={styles.actions}>
							<PopupItem
								trailing="plus"
								disabled={isAddingProject}
								onClick={() => {
									setOpen(false);
									void addLocalRepository();
								}}
							>
								{isAddingProject ? "Adding repository…" : "Add local repository"}
							</PopupItem>
						</PopupSection>
					</Popup>
				</Combobox.Positioner>
			</Combobox.Portal>
		</Combobox.Root>
	);
};
