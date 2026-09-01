import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { FolderIcon } from "#ui/components/FolderIcon.tsx";
import { PopupItem, PopupSearch, PopupSection } from "#ui/components/Popup.tsx";
import popupStyles from "#ui/components/Popup.module.css";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { useAddLocalRepository } from "#ui/components/useAddLocalRepository.ts";
import { globalHotkeys } from "#ui/hotkeys.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { readProjectsOpenedAt, writeLastOpenedProject } from "#ui/project.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Button, Combobox, Tooltip } from "@base-ui/react";
import type { ProjectForFrontend } from "@gitbutler/but-sdk";
import { useSuspenseQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useMemo, useState, type FC } from "react";
// The trigger is the header's project name, and the header's container query hides its label
// when the sidebar narrows — so the button keeps wearing the header's classes.
import headerStyles from "./SidebarHeader.module.css";
import styles from "./ProjectPicker.module.css";

/**
 * The project's name in the header, and the list of projects it opens.
 *
 * An anchored dropdown rather than a modal, built the way the commit target selector is: Base UI
 * anchors a popup to a trigger that is its own child, so the list lives with the button that
 * raises it. It is the same shape as the pickers built on `PickerDialog` minus the virtualiser,
 * which a list of a person's projects has no use for.
 */
type ProjectGroup = { value: string; items: Array<ProjectForFrontend> };

/** How many projects lead the list before the rest are folded into "Older". */
const recentCount = 5;

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

export const ProjectPicker: FC<{ project: ProjectForFrontend }> = (p) => {
	const navigate = useNavigate();
	const dispatch = useAppDispatch();
	const dialog = useAppSelector(interfaceSlice.selectors.selectDialogState);
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const { addLocalRepository, isPending: isAddingProject } = useAddLocalRepository();
	const [query, setQuery] = useState("");

	// Read once per opening rather than per render: the stamp is written on the way out of the
	// picker, so the order it produces has to hold still while the list is up.
	const open = dialog._tag === "ProjectPicker";
	const groups = useMemo(
		() => (open ? groupProjects(projects, readProjectsOpenedAt()) : []),
		[open, projects],
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
			inputValue={query}
			onInputValueChange={setQuery}
			itemToStringLabel={(project) => project.title}
			itemToStringValue={(project) => project.id}
			isItemEqualToValue={(a, b) => a.id === b.id}
			autoHighlight
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
					<Combobox.Popup
						className={classes(popupStyles.popup, popupStyles.dropdown, styles.popup)}
					>
						<PopupSearch
							placeholder="Search projects…"
							aria-label="Search projects"
							onClear={query === "" ? undefined : () => setQuery("")}
							render={<Combobox.Input />}
						/>
						<Combobox.Empty>
							<div className={classes("text-13", styles.empty)}>No projects found.</div>
						</Combobox.Empty>
						<Combobox.List className={styles.list}>
							{(group: ProjectGroup) => (
								// A group wears what `PopupSection` wears — the same three classes it composes —
								// because the section element here has to be Base UI's, to label its own rows.
								<Combobox.Group
									key={group.value}
									items={group.items}
									className={popupStyles.section}
								>
									<Combobox.GroupLabel className={classes("text-12", popupStyles.sectionLabel)}>
										{group.value}
									</Combobox.GroupLabel>
									<div className={popupStyles.sectionItems}>
										<Combobox.Collection>
											{(project: ProjectForFrontend) => (
												<PopupItem
													key={project.id}
													icon="folder"
													// The project already open is marked rather than labelled: every
													// other row would say "Project" to explain the one saying
													// "Current".
													trailing={project.id === p.project.id ? "tick" : undefined}
													render={<Combobox.Item value={project} />}
												>
													{project.title}
												</PopupItem>
											)}
										</Combobox.Collection>
									</div>
								</Combobox.Group>
							)}
						</Combobox.List>
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
					</Combobox.Popup>
				</Combobox.Positioner>
			</Combobox.Portal>
		</Combobox.Root>
	);
};
