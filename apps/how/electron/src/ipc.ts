export type SaveState = "idle" | "watching" | "pending" | "saving" | "saved" | "error";

export type ProjectSummary = {
	id: string;
	title: string;
	path: string;
	gitDir: string;
};

export type Checkpoint = {
	id: string;
	title: string;
	createdAt: number;
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
	browsing: BrowsingSession | null;
};

export type StatusEvent = HowStatus;

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
	startProject: () => Promise<OpenProjectResult>;
	deleteProject: () => Promise<HowStatus>;
	createCheckpointNow: () => Promise<HowStatus>;
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
	startProject: "how:start-project",
	deleteProject: "how:delete-project",
	createCheckpointNow: "how:create-checkpoint-now",
	viewCheckpoint: "how:view-checkpoint",
	continueFromCheckpoint: "how:continue-from-checkpoint",
	returnToLatest: "how:return-to-latest",
	status: "how:status",
} as const;
