import { changesInWorktreeQueryOptions, listProjectsQueryOptions } from "#ui/api/queries.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { lastOpenedProjectKey } from "#ui/projects/last-opened.ts";
import { PickerDialog } from "#ui/components/PickerDialog.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { classes } from "#ui/components/classes.ts";
import { globalHotkeys } from "#ui/hotkeys.ts";
import { Button, Tooltip } from "@base-ui/react";
import {
	HotkeysProvider,
	useHotkey,
	UseHotkeyDefinition,
	useHotkeys,
	useKeyHold,
} from "@tanstack/react-hotkeys";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { Outlet, useMatch, useNavigate } from "@tanstack/react-router";
import { FC, useEffect, useState } from "react";
import styles from "./RootLayout.module.css";
import { ProjectForFrontend } from "@gitbutler/but-sdk";
import { Hash } from "effect";

const projectShortcutSlots = [1, 2, 3, 4, 5, 6, 7, 8, 9] as const;
const projectShortcutRevealDelayMs = 500;

interface ProjectItemProps {
	project: ProjectForFrontend;
	isSelected: boolean;
	shortcutSlot: number | undefined;
	isModHeld: boolean;
	projectShortcutsVisible: boolean;
	onSelect: (project: ProjectForFrontend) => void;
}

const ProjectItem: FC<ProjectItemProps> = ({
	project,
	isSelected,
	shortcutSlot,
	isModHeld,
	projectShortcutsVisible,
	onSelect,
}) => {
	const hue = ((Hash.string(project.id) % 360) + 360) % 360;
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(project.id));
	const hasUncommittedChanges = (worktreeChanges?.changes.length ?? 0) > 0;

	return (
		<Tooltip.Root key={project.id}>
			<Tooltip.Trigger
				delay={0}
				aria-label={`Select project ${project.title}`}
				className={classes(
					styles.project,
					isSelected && styles.selected,
					hasUncommittedChanges && styles.hasUncommittedChanges,
				)}
				onClick={() => onSelect(project)}
				style={{ "--hue": hue }}
				// We pass `disabled` here because we want to disable the button, not
				// the tooltip. Other props should be passed above.
				render={<Button focusableWhenDisabled disabled={isSelected} />}
			>
				<div className={styles.folder}>
					<span className={classes("text-bold", styles.folderFrontText)}>
						{project.title.slice(0, 2)}
					</span>

					<div className={styles.folderFront} />

					<div className={styles.folderPaperClip}>
						<div className={styles.folderPaperLeft} />
						<div className={styles.folderPaperRight} />
					</div>

					<div className={styles.folderBack} />
				</div>
				{shortcutSlot !== undefined && (
					<span
						aria-hidden
						className={classes(
							styles.projectShortcut,
							isModHeld && projectShortcutsVisible && styles.visible,
						)}
					>
						{shortcutSlot}
					</span>
				)}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4} side="right">
					<Tooltip.Popup render={<TooltipPopup />}>{project.title}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const Projects: FC = () => {
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const navigate = useNavigate();
	const [pickerOpen, setPickerOpen] = useState(false);
	const [projectShortcutsVisible, setProjectShortcutsVisible] = useState(false);
	const isModHeld = useKeyHold(isMac ? "Meta" : "Control");
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

	useEffect(() => {
		if (!isModHeld) return;

		const timeout = window.setTimeout(() => {
			setProjectShortcutsVisible(true);
		}, projectShortcutRevealDelayMs);

		return () => {
			window.clearTimeout(timeout);
			setProjectShortcutsVisible(false);
		};
	}, [isModHeld]);

	useHotkeys(
		projectShortcutSlots.flatMap((slot): Array<UseHotkeyDefinition> => {
			const project = projects[slot - 1];

			if (!project) return [];

			return [
				{
					hotkey: `Mod+${slot}`,
					callback: () => selectProject(project),
					options: {
						meta: {
							group: "Global",
							name: `Switch to ${project.title}`,
						},
					},
				},
			];
		}),
	);

	return (
		<div className={classes(styles.projects, isMac && styles.projectsMac)}>
			{projects.map((project, index) => (
				<ProjectItem
					key={project.id}
					project={project}
					isSelected={selectedProject?.id === project.id}
					shortcutSlot={projectShortcutSlots[index]}
					isModHeld={isModHeld}
					projectShortcutsVisible={projectShortcutsVisible}
					onSelect={selectProject}
				/>
			))}

			<Tooltip.Root>
				<Tooltip.Trigger
					className={classes(getButtonClassName({ variant: "ghost" }), styles.addProjectButton)}
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
				<Projects />
			</nav>
			<main className={styles.content}>
				<Outlet />
			</main>
		</div>
	</HotkeysProvider>
);
