import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { CommandFnContext, CommandFn, useCommand } from "#ui/commands/manager.ts";
import { CommandRegistrationId } from "#ui/commands/state.ts";
import { lastOpenedProjectKey } from "#ui/projects/last-opened.ts";
import { ShortcutsBarElementContext, TopBarActionsElementContext } from "#ui/portals.tsx";
import { PickerDialog } from "#ui/ui/PickerDialog/PickerDialog.tsx";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import uiStyles from "#ui/ui/ui.module.css";
import { HotkeysProvider } from "@tanstack/react-hotkeys";
import { useSuspenseQuery } from "@tanstack/react-query";
import { Outlet, useMatch, useNavigate } from "@tanstack/react-router";
import { FC, useRef, useState } from "react";
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

	const openProjectPickerCommand = useCommand(
		() => {
			setPickerOpen(true);
		},
		{
			enabled: projects.length > 0,
			group: "Global",
			commandPalette: { label: "Select project" },
			shortcutsBar: { label: "Project" },
			hotkeys: [{ hotkey: "Mod+Shift+P" }],
		},
	);

	const selectProject = (project: ProjectForFrontend) => {
		setPickerOpen(false);
		void navigate({
			to: "/project/$id/workspace",
			params: { id: project.id },
		});
		window.localStorage.setItem(lastOpenedProjectKey, project.id);
	};

	return (
		<>
			<ShortcutButton
				aria-label="Select project"
				className={uiStyles.button}
				disabled={projects.length === 0}
				hotkeys={openProjectPickerCommand.hotkeys}
				onClick={openProjectPickerCommand.commandFn}
			>
				{selectedProject?.title ?? "Select a project"}
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
		</>
	);
};

const TopBar: FC<{
	setTopBarActionsElement: (element: HTMLDivElement | null) => void;
}> = ({ setTopBarActionsElement }) => (
	<header className={styles.topBar}>
		<ProjectSelect />
		<div ref={setTopBarActionsElement} className={styles.topBarActions} />
	</header>
);

export const RootLayout: FC = () => {
	const [topBarActionsElement, setTopBarActionsElement] = useState<HTMLDivElement | null>(null);
	const [shortcutsBarElement, setShortcutsBarElement] = useState<HTMLElement | null>(null);
	const cmdMap = useRef<Map<CommandRegistrationId, CommandFn>>(new Map());

	return (
		<HotkeysProvider>
			{/* oxlint-disable-next-line react-hooks-js/refs: Only accessed imperatively. */}
			<CommandFnContext value={cmdMap.current}>
				<TopBarActionsElementContext.Provider value={topBarActionsElement}>
					<ShortcutsBarElementContext.Provider value={shortcutsBarElement}>
						<main className={styles.layout}>
							<TopBar setTopBarActionsElement={setTopBarActionsElement} />
							<section className={styles.content}>
								<Outlet />
							</section>
							<footer ref={setShortcutsBarElement} className={styles.shortcutsBarFooter} />
						</main>
					</ShortcutsBarElementContext.Provider>
				</TopBarActionsElementContext.Provider>
			</CommandFnContext>
		</HotkeysProvider>
	);
};
