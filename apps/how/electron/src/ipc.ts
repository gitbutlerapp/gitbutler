import type { ProjectSettings } from "./settings.js";
export type { CodingAgent, ProjectSettings } from "./settings.js";

export type SaveState = "idle" | "watching" | "pending" | "saving" | "saved" | "error";

export type SharedProjectState = "unknown" | "current" | "updateAvailable" | "couldNotCheck";

export type SharedProjectStatus = {
	state: SharedProjectState;
	lastCheckedAt: number | null;
	message: string | null;
};

export type ProjectSummary = {
	id: string;
	title: string;
	path: string;
	gitDir: string;
};

export type Checkpoint = {
	id: string;
	changeId: string | null;
	title: string;
	createdAt: number;
};

export type BookmarkKind = "user" | "auto";

export type Bookmark = {
	id: string;
	name: string;
	targetCommitId: string;
	createdAt: number;
	updatedAt: number;
	kind: BookmarkKind;
	isCurrent: boolean;
};

export type BrowsingSession = {
	originalLatestCheckpointId: string;
	currentCheckpointId: string;
	checkpoints: Array<Checkpoint>;
	dirty: boolean;
	startedAt: number;
};

export type HowStatus = {
	project: ProjectSummary | null;
	saveState: SaveState;
	message: string | null;
	lastSavedAt: number | null;
	checkpoints: Array<Checkpoint>;
	bookmarks: Array<Bookmark>;
	browsing: BrowsingSession | null;
	settings: ProjectSettings;
	sharedProject: SharedProjectStatus;
};

export type StatusEvent = HowStatus;

export type PublishProjectInput = {
	githubRepositoryCloneUrl?: string;
	createGithubRepositoryName?: string;
};

export type PublishProjectResult =
	| {
			type: "published";
			status: HowStatus;
	  }
	| {
			type: "needsGithubLogin";
			status: HowStatus;
	  }
	| {
			type: "needsGithubRepository";
			status: HowStatus;
			defaultRepositoryName: string;
			repositories: Array<GithubRepositorySummary> | null;
	  }
	| {
			type: "failed";
			status: HowStatus;
	  };

export type GithubRepositorySummary = {
	id: string;
	nameWithOwner: string;
	cloneUrl: string;
	isPrivate: boolean;
};

export type GithubAccount = {
	login: string;
	avatarUrl: string | null;
};

export type GithubLoginResult =
	| {
			type: "loggedIn";
			login: string;
			avatarUrl: string | null;
	  }
	| {
			type: "failed";
			message: string;
	  };

export type GithubRepositoriesResult =
	| {
			type: "repositories";
			repositories: Array<GithubRepositorySummary>;
	  }
	| {
			type: "failed";
			message: string;
	  };

export type OpenProjectResult =
	| {
			type: "opened";
			status: HowStatus;
	  }
	| {
			type: "cancelled";
	  };

export interface HowElectronApi {
	getStatus: () => Promise<HowStatus>;
	openProject: () => Promise<OpenProjectResult>;
	openProjectPath: (projectPath: string) => Promise<OpenProjectResult>;
	startProject: () => Promise<OpenProjectResult>;
	closeProject: (options?: { discardBrowsingChanges?: boolean }) => Promise<HowStatus>;
	deleteProject: () => Promise<HowStatus>;
	createCheckpointNow: () => Promise<HowStatus>;
	createBookmark: (name: string) => Promise<HowStatus>;
	createBookmarkFromCheckpoint: (name: string, checkpointId: string) => Promise<HowStatus>;
	switchBookmark: (bookmarkId: string) => Promise<HowStatus>;
	updateBookmark: (bookmarkId: string) => Promise<HowStatus>;
	renameBookmark: (bookmarkId: string, name: string) => Promise<HowStatus>;
	deleteBookmark: (bookmarkId: string) => Promise<HowStatus>;
	publishProject: (input?: PublishProjectInput) => Promise<PublishProjectResult>;
	updateProject: () => Promise<HowStatus>;
	loginToGithub: () => Promise<GithubLoginResult>;
	getGithubAccount: () => Promise<GithubAccount | null>;
	logoutOfGithub: () => Promise<GithubAccount | null>;
	listGithubRepositories: () => Promise<GithubRepositoriesResult>;
	saveProjectSettings: (settings: ProjectSettings) => Promise<HowStatus>;
	viewCheckpoint: (
		checkpointId: string,
		options?: { discardBrowsingChanges?: boolean },
	) => Promise<HowStatus>;
	continueFromCheckpoint: () => Promise<HowStatus>;
	returnToLatest: (options?: { discardBrowsingChanges?: boolean }) => Promise<HowStatus>;
	onStatus: (callback: (status: StatusEvent) => void) => () => void;
	platform: string;
}

export const howIpcChannels = {
	getStatus: "how:get-status",
	openProject: "how:open-project",
	openProjectPath: "how:open-project-path",
	startProject: "how:start-project",
	closeProject: "how:close-project",
	deleteProject: "how:delete-project",
	createCheckpointNow: "how:create-checkpoint-now",
	createBookmark: "how:create-bookmark",
	createBookmarkFromCheckpoint: "how:create-bookmark-from-checkpoint",
	switchBookmark: "how:switch-bookmark",
	updateBookmark: "how:update-bookmark",
	renameBookmark: "how:rename-bookmark",
	deleteBookmark: "how:delete-bookmark",
	publishProject: "how:publish-project",
	updateProject: "how:update-project",
	loginToGithub: "how:login-to-github",
	getGithubAccount: "how:get-github-account",
	logoutOfGithub: "how:logout-of-github",
	listGithubRepositories: "how:list-github-repositories",
	saveProjectSettings: "how:save-project-settings",
	viewCheckpoint: "how:view-checkpoint",
	continueFromCheckpoint: "how:continue-from-checkpoint",
	returnToLatest: "how:return-to-latest",
	status: "how:status",
} as const;
