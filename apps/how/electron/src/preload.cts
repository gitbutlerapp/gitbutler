import { contextBridge, ipcRenderer } from "electron";
import type {
	HowElectronApi,
	HowStatus,
	OpenProjectResult,
	PublishProjectResult,
	StatusEvent,
	GithubLoginResult,
	GithubRepositoriesResult,
} from "./ipc";

const api: HowElectronApi = {
	getStatus: async () => await (ipcRenderer.invoke("how:get-status") as Promise<HowStatus>),
	openProject: async () =>
		await (ipcRenderer.invoke("how:open-project") as Promise<OpenProjectResult>),
	openProjectPath: async (projectPath) =>
		await (ipcRenderer.invoke("how:open-project-path", projectPath) as Promise<OpenProjectResult>),
	startProject: async () =>
		await (ipcRenderer.invoke("how:start-project") as Promise<OpenProjectResult>),
	closeProject: async (options) =>
		await (ipcRenderer.invoke("how:close-project", options) as Promise<HowStatus>),
	deleteProject: async () => await (ipcRenderer.invoke("how:delete-project") as Promise<HowStatus>),
	createCheckpointNow: async () =>
		await (ipcRenderer.invoke("how:create-checkpoint-now") as Promise<HowStatus>),
	createBookmark: async (name) =>
		await (ipcRenderer.invoke("how:create-bookmark", name) as Promise<HowStatus>),
	createBookmarkFromCheckpoint: async (name, checkpointId) =>
		await (ipcRenderer.invoke(
			"how:create-bookmark-from-checkpoint",
			name,
			checkpointId,
		) as Promise<HowStatus>),
	switchBookmark: async (bookmarkId) =>
		await (ipcRenderer.invoke("how:switch-bookmark", bookmarkId) as Promise<HowStatus>),
	updateBookmark: async (bookmarkId) =>
		await (ipcRenderer.invoke("how:update-bookmark", bookmarkId) as Promise<HowStatus>),
	renameBookmark: async (bookmarkId, name) =>
		await (ipcRenderer.invoke("how:rename-bookmark", bookmarkId, name) as Promise<HowStatus>),
	deleteBookmark: async (bookmarkId) =>
		await (ipcRenderer.invoke("how:delete-bookmark", bookmarkId) as Promise<HowStatus>),
	publishProject: async (input) =>
		await (ipcRenderer.invoke("how:publish-project", input) as Promise<PublishProjectResult>),
	updateProject: async () => await (ipcRenderer.invoke("how:update-project") as Promise<HowStatus>),
	loginToGithub: async () =>
		await (ipcRenderer.invoke("how:login-to-github") as Promise<GithubLoginResult>),
	getGithubAccount: async () =>
		await (ipcRenderer.invoke("how:get-github-account") as Promise<
			Awaited<ReturnType<HowElectronApi["getGithubAccount"]>>
		>),
	logoutOfGithub: async () =>
		await (ipcRenderer.invoke("how:logout-of-github") as Promise<
			Awaited<ReturnType<HowElectronApi["logoutOfGithub"]>>
		>),
	listGithubRepositories: async () =>
		await (ipcRenderer.invoke("how:list-github-repositories") as Promise<GithubRepositoriesResult>),
	saveProjectSettings: async (settings) =>
		await (ipcRenderer.invoke("how:save-project-settings", settings) as Promise<HowStatus>),
	viewCheckpoint: async (checkpointId, options) =>
		await (ipcRenderer.invoke("how:view-checkpoint", checkpointId, options) as Promise<HowStatus>),
	continueFromCheckpoint: async () =>
		await (ipcRenderer.invoke("how:continue-from-checkpoint") as Promise<HowStatus>),
	returnToLatest: async (options) =>
		await (ipcRenderer.invoke("how:return-to-latest", options) as Promise<HowStatus>),
	onStatus: (callback) => {
		function listener(_event: Electron.IpcRendererEvent, status: StatusEvent) {
			callback(status);
		}
		ipcRenderer.on("how:status", listener);
		return () => ipcRenderer.removeListener("how:status", listener);
	},
	platform: process.platform,
};

contextBridge.exposeInMainWorld("how", api);
