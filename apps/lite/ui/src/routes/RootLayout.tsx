import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { lastOpenedProjectKey } from "#ui/projects/last-opened.ts";
import { PickerDialog } from "#ui/components/PickerDialog.tsx";
import { ProjectButton } from "#ui/components/ProjectButton.tsx";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import { globalHotkeys } from "#ui/hotkeys.ts";
import { HotkeysProvider, useHotkey } from "@tanstack/react-hotkeys";
import { useSuspenseQuery } from "@tanstack/react-query";
import { Outlet, useMatch, useNavigate } from "@tanstack/react-router";
import { FC, useState } from "react";
import styles from "./RootLayout.module.css";
import { ProjectForFrontend } from "@gitbutler/but-sdk";

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

	return (
		<div className={styles.projects}>
			{projects.map((project) => {
				const isSelected = selectedProject?.id === project.id;

				return (
					<ProjectButton
						key={project.id}
						title={project.title}
						id={project.id}
						isSelected={isSelected}
						onClick={() => selectProject(project)}
					/>
				);
			})}

			<ShortcutButton
				aria-label="Select project"
				variant="ghost"
				hotkey={globalHotkeys.selectProject.hotkey}
				className={styles.addProjectButton}
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

const isMac = window.lite.platform === "darwin";

export const RootLayout: FC = () => (
	<HotkeysProvider>
		<div className={styles.layout}>
			<div className={styles.dragRegion} />
			<nav className={styles.sidebar}>
				{isMac && <div className={styles.sidebarMacSpacer} />}
				<div
					className={[styles.sidebarScroll, isMac ? styles.sidebarScrollMac : undefined]
						.filter(Boolean)
						.join(" ")}
				>
					<ProjectSelect />
				</div>
			</nav>
			<main className={styles.content}>
				<Outlet />
			</main>
		</div>
	</HotkeysProvider>
);
