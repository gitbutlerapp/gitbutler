import { goto } from "$app/navigation";
import { showError } from "$lib/error/showError";
import { handleAddProjectOutcome, type Project } from "$lib/project/project";
import { projectPath } from "$lib/routes/routes.svelte";
import { getCookie } from "$lib/utils/cookies";
import { InjectionToken } from "@gitbutler/core/context";
import { persisted } from "@gitbutler/shared/persisted";
import { chipToasts } from "@gitbutler/ui";
import { get } from "svelte/store";
import type { IBackend } from "$lib/backend";
import type { ProjectInfo } from "$lib/project/projectEndpoints";
import type { BackendApi } from "$lib/state/clientState.svelte";
import type { ForgeUser } from "@gitbutler/core/api";

export type { ProjectInfo, ServerInfo } from "$lib/project/projectEndpoints";

export const PROJECTS_SERVICE = new InjectionToken<ProjectsService>("ProjectsService");

export class ProjectsService {
	private persistedId = persisted<string | undefined>(undefined, "lastProject");

	constructor(
		private backendApi: BackendApi,
		private homeDir: string | undefined,
		private backend: IBackend,
	) {}

	serverInfo() {
		return this.backendApi.endpoints.serverInfo.useQuery();
	}

	projects() {
		return this.backendApi.endpoints.listProjects.useQuery();
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
		if (/^\\\\wsl.localhost/i.test(path)) {
			const errorMsg =
				"For WSL2 projects, install the Linux version of GitButler inside of your WSL2 distro";
			console.error(errorMsg);
			showError("Use the Linux version of GitButler", errorMsg);

			return false;
		}

		if (/^\\\\/i.test(path)) {
			const errorMsg =
				"Using git across a network is not recommended. Either clone " +
				"the repo locally, or use the NET USE command to map a " +
				"network drive";
			console.error(errorMsg);
			showError("UNC Paths are not directly supported", errorMsg);

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
