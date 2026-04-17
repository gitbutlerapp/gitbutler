import { goto } from "$app/navigation";
import { showError } from "$lib/error/showError";
import { showToast } from "$lib/notifications/toasts";
import { handleAddProjectOutcome, type Project } from "$lib/project/project";
import { projectPath } from "$lib/routes/routes.svelte";
import { getCookie } from "$lib/utils/cookies";
import { InjectionToken } from "@gitbutler/core/context";
import { persisted } from "@gitbutler/shared/persisted";
import { chipToasts } from "@gitbutler/ui";
import { get } from "svelte/store";
import type { IBackend } from "$lib/backend";
import type { ProjectInfo } from "$lib/project/projectEndpoints";
import type { BackendApi } from "$lib/state/backendApi";
import type { ForgeUser } from "@gitbutler/but-sdk";

export const PROJECTS_SERVICE = new InjectionToken<ProjectsService>("ProjectsService");

export class ProjectsService {
	private persistedId = persisted<string | undefined>(undefined, "lastProject");

	constructor(
		private backendApi: BackendApi,
		private homeDir: string | undefined,
		private backend: IBackend,
	) {}

	projects() {
		return this.backendApi.endpoints.listProjects.useQuery();
	}

	async fetchProjects() {
		return await this.backendApi.endpoints.listProjects.fetch();
	}

	/**
	 * Capabilities that vary by how the backend was launched (local Tauri app
	 * vs. but-server running behind a tunnel). Used to hide UI entry points
	 * that require the user to be on the same machine as the backend.
	 */
	serverCapabilities() {
		return this.backendApi.endpoints.serverCapabilities.useQuery();
	}

	getProject(projectId: string, noValidation?: boolean) {
		return this.backendApi.endpoints.project.useQuery({ projectId, noValidation });
	}

	async fetchProject(projectId: string, noValidation?: boolean) {
		return await this.backendApi.endpoints.project.fetch({ projectId, noValidation });
	}

	async setActiveProject(projectId: string): Promise<ProjectInfo | null> {
		return await this.backendApi.endpoints.setProjectActive.mutate({ id: projectId });
	}

	async updateProject(project: Project & { unset_bool?: boolean; unset_forge_override?: boolean }) {
		await this.backendApi.endpoints.updateProject.mutate({ project });
	}

	async updatePreferredForgeUser(projectId: string, preferredForgeUser: ForgeUser | null) {
		const project = await this.fetchProject(projectId, true);

		await this.updateProject({
			...project,
			preferred_forge_user: preferredForgeUser,
		});
	}

	async deleteProject(projectId: string) {
		const response = await this.backendApi.endpoints.deleteProject.mutate({ projectId });
		if (this.getLastOpenedProject() === projectId) {
			this.unsetLastOpenedProject();
		}
		return response;
	}

	/**
	 * Whether this project is configured to use Gerrit as per its Git config.
	 *
	 * This is different from checking for signals of Gerrit usage. This tells us whether the
	 * user has explicitly set the Gerrit mode for this project.
	 * Default value is false.
	 *
	 * @see {areYouGerritKiddingMe} for checking for signals of Gerrit usage.
	 */
	isGerritProject(projectId: string) {
		return this.backendApi.endpoints.project.useQuery(
			{ projectId, noValidation: true },
			{ transform: (data) => data.gerrit_mode },
		);
	}

	async promptForDirectory(): Promise<string | undefined> {
		const cookiePath = getCookie("test-projectPath");
		if (cookiePath) {
			return cookiePath;
		}
		const selectedPath = await this.backend.filePicker({
			directory: true,
			recursive: true,
			defaultPath: this.homeDir,
		});
		if (selectedPath) {
			return selectedPath;
		}
	}

	// TODO: Reinstate the ability to open a project in a new window.
	async openProjectInNewWindow(projectId: string) {
		await this.backendApi.endpoints.openProjectInWindow.mutate({ id: projectId });
	}

	async relocateProject(projectId: string): Promise<void> {
		const path = await this.getValidPath();
		if (!path) return;

		try {
			const project = await this.fetchProject(projectId, true);
			await this.updateProject({ ...project, path });
			chipToasts.success(`Project ${project.title} relocated`);
			window.location.reload();
		} catch (error: any) {
			showError("Failed to relocate project:", error.message);
		}
	}

	async addProject(path?: string) {
		const capabilities = await this.backendApi.endpoints.serverCapabilities.fetch();
		if (!capabilities?.canAddProjects) {
			showToast({
				style: "info",
				title: "Adding projects is disabled",
				message: "Projects can only be added when GitButler runs on your local machine.",
			});
			return;
		}
		if (!path) {
			path = await this.getValidPath();
			if (!path) return;
		}
		return await this.backendApi.endpoints.addProject.mutate({ path });
	}

	async handleDeepLinkOpen(path: string) {
		const outcome = await this.backendApi.endpoints.addProjectWithBestEffort.mutate({ path });
		if (outcome) {
			switch (outcome.type) {
				case "added":
				case "alreadyExists":
					goto(projectPath(outcome.subject.id));
					break;
				default:
					handleAddProjectOutcome(outcome);
			}
		}
	}

	async getValidPath(): Promise<string | undefined> {
		const path = await this.promptForDirectory();
		if (!path) return undefined;
		if (!this.validateProjectPath(path)) return undefined;
		return path;
	}

	validateProjectPath(path: string) {
		// These two paths represent unsupported-configuration guidance, not
		// runtime errors. Surface them as info toasts so they don't pollute
		// error telemetry — they previously accounted for 53 + many events
		// of noisy toast:show_error captures.
		if (/^\\\\wsl.localhost/i.test(path)) {
			const message =
				"For WSL2 projects, install the Linux version of GitButler inside of your WSL2 distro.";
			console.warn(message);
			showToast({
				style: "info",
				title: "Use the Linux version of GitButler",
				message,
			});

			return false;
		}

		if (/^\\\\/i.test(path)) {
			const message =
				"Using git across a network is not recommended. Either clone " +
				"the repo locally, or use the NET USE command to map a " +
				"network drive.";
			console.warn(message);
			showToast({
				style: "info",
				title: "UNC paths are not directly supported",
				message,
			});

			return false;
		}

		return true;
	}

	getLastOpenedProject() {
		return get(this.persistedId);
	}

	setLastOpenedProject(projectId: string) {
		this.persistedId.set(projectId);
	}

	unsetLastOpenedProject() {
		this.persistedId.set(undefined);
	}

	/**
	 * Check if the project's repository is potentially using Gerrit.
	 *
	 * This is different from querying the repository config value, as it only
	 * checks for signals of a Gerrit setup.
	 *
	 * @see {isGerritProject} for checking the actual config value.
	 */
	areYouGerritKiddingMe(projectId: string) {
		return this.backendApi.endpoints.areYouGerritKiddingMe.useQuery({ projectId });
	}
}
