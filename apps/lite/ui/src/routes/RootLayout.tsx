import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { lastOpenedProjectKey } from "#ui/projects/last-opened.ts";
import { PickerDialog } from "#ui/components/PickerDialog.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { classes } from "#ui/components/classes.ts";
import { globalHotkeys } from "#ui/hotkeys.ts";
import { Tooltip } from "@base-ui/react";
import { HotkeysProvider, useHotkey } from "@tanstack/react-hotkeys";
import { useSuspenseQuery } from "@tanstack/react-query";
import { Outlet, useMatch, useNavigate } from "@tanstack/react-router";
import { FC, useState } from "react";
import styles from "./RootLayout.module.css";
import { ProjectForFrontend } from "@gitbutler/but-sdk";
import { Hash } from "effect";

const Projects: FC = () => {
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const navigate = useNavigate();
	const [pickerOpen, setPickerOpen] = useState(false);
	const projectMatch = useMatch({
		from: "/project/$id",
		shouldThrow: false,
	});
	const selectedProjectId = projectMatch?.params.id;
	const selectedProject = projects.find((project) => project.id === selectedProjectId);

	const openProjectPicker = () => {
		setPickerOpen(true);
	};

	useHotkey(globalHotkeys.selectProject.hotkey, openProjectPicker, {
		enabled: projects.length > 0,
		meta: globalHotkeys.selectProject.meta,
	});

	const selectProject = (project: ProjectForFrontend) => {
		setPickerOpen(false);
		void navigate({
			to: "/project/$id/workspace",
			params: { id: project.id },
		});
		window.localStorage.setItem(lastOpenedProjectKey, project.id);
	};

	return (
		<div className={styles.projects}>
			{projects.map((project) => {
				const isSelected = selectedProject?.id === project.id;
				const hue = ((Hash.string(project.id) % 360) + 360) % 360;

				return (
					<Tooltip.Root key={project.id}>
						<Tooltip.Trigger
							aria-label={`Select project ${project.title}`}
							className={classes(styles.project, isSelected && styles.selected)}
							onClick={() => selectProject(project)}
							style={{ "--hue": hue }}
							disabled={isSelected}
						>
							<div className={styles.folderFront}>
								<span className={classes("text-bold", styles.folderFrontText)}>
									{project.title.slice(0, 2)}
								</span>
							</div>
						</Tooltip.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4} side="right">
								<Tooltip.Popup render={<TooltipPopup />}>{project.title}</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				);
			})}

			<Tooltip.Root>
				<Tooltip.Trigger
					className={classes(
						getButtonClassName({ variant: "ghost", iconOnly: true }),
						styles.addProjectButton,
					)}
					aria-label="Select project"
					onClick={openProjectPicker}
				>
					<Icon name="plus" />
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4} side="right">
						<Tooltip.Popup render={<TooltipPopup kbd={globalHotkeys.selectProject.hotkey} />}>
							{globalHotkeys.selectProject.meta.name}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>

			<PickerDialog
				ariaLabel="Select project"
				closeLabel="Close project picker"
				emptyLabel="No projects found."
				getItemKey={(project) => project.id}
				getItemLabel={(project) => project.title}
				getItemType={(project) => (project.id === selectedProjectId ? "Current" : "Project")}
				itemToStringValue={(project) => project.title}
				items={[
					{
						value: "Projects",
						items: projects,
					},
				]}
				open={pickerOpen}
				onOpenChange={setPickerOpen}
				onSelectItem={selectProject}
				placeholder="Search projects…"
			/>
		</div>
	);
};

const isMac = window.lite.platform === "darwin";

export const RootLayout: FC = () => (
	<HotkeysProvider>
		<div className={styles.dragRegion} />
		<div className={styles.layout}>
			<nav className={styles.sidebar}>
				{isMac && <div className={styles.sidebarMacSpacer} />}
				<div className={styles.sidebarScroll}>
					<Projects />
				</div>
			</nav>
			<main className={styles.content}>
				<Outlet />
			</main>
		</div>
	</HotkeysProvider>
);
