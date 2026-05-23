import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { lastOpenedProjectKey } from "#ui/projects/last-opened.ts";
import { PickerDialog } from "#ui/components/PickerDialog/PickerDialog.tsx";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import { globalHotkeys } from "#ui/hotkeys.ts";
import uiStyles from "#ui/components/ui.module.css";
import { HotkeysProvider, useHotkey } from "@tanstack/react-hotkeys";
import { useSuspenseQuery } from "@tanstack/react-query";
import { Outlet, useMatch, useNavigate } from "@tanstack/react-router";
import { FC, useState } from "react";
import styles from "./RootLayout.module.css";
import { ProjectForFrontend } from "@gitbutler/but-sdk";
import { classes } from "#ui/components/classes.ts";
import { Hash } from "effect";
import { Tooltip } from "@base-ui/react";

const ProjectSelect: FC = () => {
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

	const hue = (id: string): number => ((Hash.string(id) % 360) + 360) % 360;

	return (
		<div className={styles.projects}>
			{projects.map((project) => {
				const isSelected = selectedProject?.id === project.id;

				return (
					<Tooltip.Root key={project.id}>
						<Tooltip.Trigger
							aria-label={`Select project ${project.title}`}
							className={classes(uiStyles.button, styles.project, isSelected && styles.selected)}
							onClick={() => selectProject(project)}
							style={{ "--hue": hue(project.id) }}
							render={<button type="button" disabled={isSelected} />}
						>
							{project.title.slice(0, 2)}
						</Tooltip.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner side="right" sideOffset={8}>
								<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip)}>
									{project.title}
								</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				);
			})}

			<ShortcutButton
				aria-label="Select project"
				variant="ghost"
				className={classes(styles.picker)}
				hotkey={globalHotkeys.selectProject.hotkey}
				hotkeyOptions={{ meta: globalHotkeys.selectProject.meta }}
				onClick={openProjectPicker}
				positionerProps={{ side: "right" }}
			>
				<Icon name="plus" />
			</ShortcutButton>

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

export const RootLayout: FC = () => (
	<HotkeysProvider>
		<div className={styles.layout}>
			<nav className={styles.sidebar}>
				<ProjectSelect />
			</nav>
			<main className={styles.content}>
				<Outlet />
			</main>
		</div>
	</HotkeysProvider>
);
