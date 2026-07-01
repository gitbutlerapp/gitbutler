import { type SnapPositionName } from "$lib/floating/types";
import { InjectionToken } from "@gitbutler/core/context";
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
import { type Reactive } from "@gitbutler/shared/storeUtils";
import { createEntityAdapter, createSlice, type EntityState } from "@reduxjs/toolkit";
import type { TerminalService } from "$lib/settings/terminalService";
import type { AppDispatch } from "$lib/state/clientState.svelte";
import type { ScrollbarVisilitySettings } from "@gitbutler/ui";

export type GeneralSettingsPageId =
	| "general"
	| "appearance"
	| "lanes-and-branches"
	| "git"
	| "integrations"
	| "ai"
	| "irc"
	| "telemetry"
	| "experimental"
	| "organizations";
export type ProjectSettingsPageId = "project" | "git" | "ai" | "experimental";
export type RejectionReason =
	| "workspaceMergeConflict"
	| "workspaceMergeConflictOfUnrelatedFile"
	| "cherryPickMergeConflict"
	| "noEffectiveChanges"
	| "worktreeFileMissingForObjectConversion"
	| "fileToLargeOrBinary"
	| "pathNotFoundInBaseTree"
	| "unsupportedDirectoryEntry"
	| "unsupportedTreeEntry"
	| "missingDiffSpecAssociation";

export type StackSelection = {
	branchName?: string;
	/** The primary selected commit (drives the preview pane). */
	commitId?: string;
	/** All selected commit IDs (for multi-select). When undefined, only `commitId` is selected. */
	commitIds?: string[];
	upstream?: boolean;
	previewOpen: boolean;
	irc?: boolean;
};

export type NewCommitMessage = {
	title: string;
	description: string;
};

export type StackState = {
	selection: StackSelection | undefined;
	newCommitMessage: NewCommitMessage;
};

export type ExclusiveAction =
	| {
			type: "commit";
			stackId: string | undefined;
			branchName: string | undefined;
			parentCommitId?: string;
			insertBelow?: boolean;
	  }
	| {
			type: "edit-commit-message";
			stackId: string | undefined;
			branchName: string | undefined;
			commitId: string;
	  }
	| {
			type: "create-pr";
			stackId: string | undefined;
			branchName: string;
	  };

export type StackBusyState = {
	commitId?: string;
	stackIds?: string[];
};

export type ProjectUiState = {
	exclusiveAction: ExclusiveAction | undefined;
	stackBusy: StackBusyState | undefined;
	branchesToPoll: string[];
};

type GlobalModalType =
	| "commit-failed"
	| "author-missing"
	| "general-settings"
	| "project-settings"
	| "login-confirmation";
type BaseGlobalModalState = {
	type: GlobalModalType;
};

export type CommitFailedModalState = BaseGlobalModalState & {
	type: "commit-failed";
	projectId: string;
	targetBranchName: string;
	newCommitId: string | undefined;
	commitTitle: string | undefined;
	pathsToRejectedChanges: Record<string, RejectionReason>;
};

export type AuthorMissingModalState = BaseGlobalModalState & {
	type: "author-missing";
	projectId: string;
	authorName: string | undefined;
	authorEmail: string | undefined;
};

export type GeneralSettingsModalState = BaseGlobalModalState & {
	type: "general-settings";
	selectedId?: GeneralSettingsPageId;
};

export type ProjectSettingsModalState = BaseGlobalModalState & {
	type: "project-settings";
	projectId: string;
	selectedId?: ProjectSettingsPageId;
};

export type LoginConfirmationModalState = BaseGlobalModalState & {
	type: "login-confirmation";
};

export type AppTheme = "system" | "light" | "dark";

export type GlobalModalState =
	| CommitFailedModalState
	| AuthorMissingModalState
	| GeneralSettingsModalState
	| ProjectSettingsModalState
	| LoginConfirmationModalState;

export type CodeEditorSettings = {
	schemeIdentifer: string;
	displayName: string;
};

export type TerminalSettings = {
	identifier: string;
	displayName: string;
	platform: "macos" | "windows" | "linux";
};

export type GlobalUiState = {
	drawerHeight: number;
	stackWidth: number;
	detailsWidth: number;
	previewWidth: number;
	useFloatingBox: boolean;
	floatingBoxSize: {
		width: number;
		height: number;
	};
	floatingBoxPosition: SnapPositionName;
	unassignedSidebarFolded: boolean;
	useRuler: boolean;
	rulerCountValue: number;
	aiSuggestionsOnType: boolean;
	ircChatOpen: boolean;
	ircChatSize: {
		width: number;
		height: number;
	};
	ircChatXY: { x: number; y: number } | undefined;
	channel: string | undefined;
	draftBranchName: string | undefined;
	modal: GlobalModalState | undefined;
	// User settings (migrated from userSettings.ts)
	aiSummariesEnabled: boolean;
	bottomPanelExpanded: boolean;
	bottomPanelHeight: number;
	peekTrayWidth: number;
	theme: AppTheme;
	trayWidth: number;
	stashedBranchesHeight: number;
	defaultLaneWidth: number;
	defaultFileWidth: number;
	defaultTreeHeight: number;
	zoom: number;
	scrollbarVisibilityState: ScrollbarVisilitySettings;
	tabSize: number;
	wrapText: boolean;
	diffFont: string;
	diffFontSize: number;
	diffLigatures: boolean;
	inlineUnifiedDiffs: boolean;
	strongContrast: boolean;
	colorBlindFriendly: boolean;
	defaultCodeEditor: CodeEditorSettings;
	defaultTerminal: TerminalSettings;
	defaultFileListMode: "tree" | "list";
	pathFirst: boolean;
	allInOneDiff: boolean;
	highlightDiffs: boolean;
	svgAsImage: boolean;
	syntaxThemeLight: string;
	syntaxThemeDark: string;
};

export const UI_STATE = new InjectionToken<UiState>("UiState");

/**
 * Stateful properties for the UI, with redux backed fine-grained reactivity.
 */
export class UiState {
	private state = $state.raw<EntityState<UiStateVariable, string>>(uiStateSlice.getInitialState());

	private scopesCache = {
		lanes: {} as Record<string, WritableReactiveStore<any>>,
		projects: {} as Record<string, WritableReactiveStore<any>>,
	};

	/** Properties scoped to a specific stack. */
	readonly lane = this.buildScopedProps<StackState>(this.scopesCache.lanes, {
		selection: undefined,
		newCommitMessage: { title: "", description: "" },
	});

	/** Properties scoped to a specific project. */
	readonly project = this.buildScopedProps<ProjectUiState>(this.scopesCache.projects, {
		exclusiveAction: undefined,
		stackBusy: undefined,
		branchesToPoll: [],
	});

	/** Properties that are globally scoped. */
	readonly global = this.buildGlobalProps<GlobalUiState>({
		drawerHeight: 20,
		stackWidth: 22.5,
		detailsWidth: 32,
		previewWidth: 48,
		useFloatingBox: false,
		floatingBoxSize: {
			width: 640,
			height: 330,
		},
		floatingBoxPosition: "bottom-center",
		unassignedSidebarFolded: false,
		useRuler: true,
		rulerCountValue: 72,
		aiSuggestionsOnType: false,
		ircChatOpen: false,
		ircChatSize: {
			width: 520,
			height: 460,
		},
		ircChatXY: undefined,
		channel: undefined,
		draftBranchName: undefined,
		modal: undefined,
		// User settings defaults
		aiSummariesEnabled: false,
		bottomPanelExpanded: false,
		bottomPanelHeight: 200,
		peekTrayWidth: 480,
		theme: "system",
		trayWidth: 320,
		stashedBranchesHeight: 150,
		defaultLaneWidth: 460,
		defaultFileWidth: 460,
		defaultTreeHeight: 100,
		zoom: 1,
		scrollbarVisibilityState: "scroll",
		tabSize: 4,
		wrapText: false,
		diffFont: "Geist Mono, Menlo, monospace",
		diffFontSize: 12,
		diffLigatures: false,
		inlineUnifiedDiffs: false,
		strongContrast: false,
		colorBlindFriendly: false,
		defaultCodeEditor: { schemeIdentifer: "vscode", displayName: "VSCode" },
		defaultTerminal: { identifier: "terminal", displayName: "Terminal", platform: "macos" },
		defaultFileListMode: "list",
		pathFirst: true,
		allInOneDiff: false,
		highlightDiffs: false,
		svgAsImage: true,
		syntaxThemeLight: "github-light",
		syntaxThemeDark: "github-dark",
	});

	/**
	 * Returns a reactive proxy for the given global property keys. Property
	 * accesses are forwarded to `.current` via getters, so values stay in
	 * sync with the store rather than being captured at call time.
	 *
	 * Useful for spreading into component props:
	 * ```svelte
	 * <HunkDiff {...uiState.pick('tabSize', 'wrapText', 'diffFont')} />
	 * ```
	 */
	pick<K extends keyof GlobalUiState>(...keys: K[]): { [P in K]: GlobalUiState[P] } {
		const result = {} as { [P in K]: GlobalUiState[P] };
		for (const key of keys) {
			Object.defineProperty(result, key, {
				get: () => this.global[key].current,
				enumerable: true,
			});
		}
		return result;
	}

	constructor(
		reactiveState: Reactive<typeof this.state>,
		private dispatch: AppDispatch,
	) {
		$effect(() => {
			this.state = reactiveState.current;
		});
	}

	private getById(id: string, defaultValue: UiStateValue): Reactive<UiStateValue> {
		const item = $derived(selectById(this.state, id));
		return reactive(() => (item?.value !== undefined ? item.value : defaultValue));
	}

	private update(id: string, value: UiStateValue) {
		this.dispatch(upsertOne({ id, value }));
	}

	/**
	 * Creates a single redux-backed reactive property with get/set and
	 * optional convenience methods for arrays and objects.
	 */
	private createProperty(id: string, defaultValue: UiStateValue): WritableReactive<UiStateValue> {
		const result = this.getById(id, defaultValue);
		let mutableResult = $derived(result.current);

		const prop: Record<string, unknown> = {
			set: (value: UiStateValue) => {
				mutableResult = value;
				this.update(id, value);
			},
			get current() {
				return mutableResult;
			},
		};

		if (Array.isArray(defaultValue)) {
			prop.add = (...value: string[]) => {
				const current = mutableResult as string[];
				const toAdd = value.filter((v) => !current.includes(v));
				if (toAdd.length === 0) return;
				mutableResult = [...current, ...toAdd];
				this.update(id, mutableResult);
			};
			prop.remove = (value: string) => {
				mutableResult = (mutableResult as string[]).filter((v) => v !== value);
				this.update(id, mutableResult);
			};
		} else if (typeof defaultValue === "object" && defaultValue !== null) {
			prop.update = (value: Record<string, UiStateValue>) => {
				mutableResult = { ...(mutableResult as Record<string, UiStateValue>), ...value };
				this.update(id, mutableResult);
			};
		}

		return prop as WritableReactive<UiStateValue>;
	}

	/**
	 * Generate redux backed properties corresponding to the shape of the
	 * parameter value, with types corresponding to their default values.
	 */
	private buildGlobalProps<T extends DefaultConfig>(param: T): WritableReactiveStore<T> {
		const props: WritableReactiveStore<DefaultConfig> = {};
		for (const [key, defaultValue] of Object.entries(param)) {
			props[key] = this.createProperty(key, defaultValue);
		}
		return props as WritableReactiveStore<T>;
	}

	/**
	 * Scoped props are the same as global, except they take an additional
	 * parameter for the key. This allows us to define values that are scoped
	 * to e.g. a projectId.
	 */
	private buildScopedProps<T extends DefaultConfig>(
		scopeCache: Record<string, WritableReactiveStore<T>>,
		defaultConfig: T,
	): (id: string) => WritableReactiveStore<T> {
		return (id: string) => {
			if (id in scopeCache) {
				return scopeCache[id] as WritableReactiveStore<T>;
			}
			const props: WritableReactiveStore<DefaultConfig> = {};
			for (const [key, defaultValue] of Object.entries(defaultConfig)) {
				props[key] = this.createProperty(`${id}:${key}`, defaultValue);
			}
			scopeCache[id] = props as WritableReactiveStore<T>;
			return scopeCache[id];
		};
	}
}

export const uiStateAdapter = createEntityAdapter<UiStateVariable, string>({
	selectId: (item) => item.id,
});

const { selectById } = uiStateAdapter.getSelectors();

export const uiStateSlice = createSlice({
	name: "uiState",
	initialState: uiStateAdapter.getInitialState(),
	reducers: {
		upsertOne: uiStateAdapter.upsertOne,
	},
	selectors: { selectById },
});

const { upsertOne } = uiStateSlice.actions;

/** Allowed types for property values. */
type UiStateValue =
	| string
	| string[]
	| number
	| boolean
	| { [property: string]: UiStateValue }
	| undefined;

/** Type held by the RTK entity adapter. */
type UiStateVariable = {
	id: string;
	value: UiStateValue;
};

/** Shape of the config expected by the build functions. */
type DefaultConfig = Record<string, UiStateValue>;

type ArrayPropertyMethods<T> = T extends string[]
	? {
			/** Will not add the value if it already exists in the array. */
			add(...value: string[]): void;
			/** Removes the value from the array. */
			remove(value: string): void;
		}
	: // eslint-disable-next-line @typescript-eslint/no-empty-object-type
		{};
type ObjectPropertyMethods<T> =
	T extends Record<string, UiStateValue>
		? {
				/** Updates the object with the new values, keeps existing values. */
				update(value: Record<string, UiStateValue>): void;
			}
		: // eslint-disable-next-line @typescript-eslint/no-empty-object-type
			{};

/** A reactive value that can be read (.current) and written (.set). */
export type WritableReactive<T> = {
	set(value: T): void;
} & Reactive<T> &
	ArrayPropertyMethods<T> &
	ObjectPropertyMethods<T>;

/** A record of WritableReactive properties keyed by the config shape. */
export type WritableReactiveStore<T extends DefaultConfig> = {
	[K in keyof T]: WritableReactive<T[K]>;
};

const LEGACY_SETTINGS_KEY = "settings-json";

/**
 * Initialises user settings in UiState:
 * 1. Migrates any legacy `settings-json` localStorage data into the Redux-backed store.
 * 2. Ensures the default terminal matches the current platform (the static
 *    default is macOS; this corrects it for Windows/Linux on fresh installs).
 *
 * Must be called after Redux Persist has rehydrated so that migrated values
 * are not overwritten.
 */
export async function initUserSettings(
	uiState: UiState,
	platformName: string,
	terminalService: TerminalService,
) {
	const raw = localStorage.getItem(LEGACY_SETTINGS_KEY);
	if (raw) {
		try {
			const obj = JSON.parse(raw) as Record<string, unknown>;
			const g = uiState.global;
			for (const [key, value] of Object.entries(obj)) {
				if (!Object.prototype.hasOwnProperty.call(g, key)) continue;
				const prop = g[key as keyof typeof g];
				if (!prop || value === null || value === undefined) continue;
				// Validate that the migrated value's type matches the current default
				// to avoid persisting e.g. a string where an object is expected.
				if (typeof value !== typeof prop.current) continue;
				(prop as WritableReactive<UiStateValue>).set(value as UiStateValue);
			}
			localStorage.removeItem(LEGACY_SETTINGS_KEY);
		} catch {
			// Corrupted data – just remove it.
			localStorage.removeItem(LEGACY_SETTINGS_KEY);
		}
	}

	// Ensure the terminal default matches the platform.
	const DESKTOP_PLATFORMS = ["macos", "windows", "linux"];
	if (DESKTOP_PLATFORMS.includes(platformName)) {
		const terminal = uiState.global.defaultTerminal.current as TerminalSettings | null;
		if (terminal?.platform !== platformName) {
			try {
				const recommended = await terminalService.getRecommendedTerminalForPlatform(platformName);
				const fallback =
					recommended ?? (await terminalService.getTerminalOptionsForPlatform(platformName))[0];
				if (fallback) {
					uiState.global.defaultTerminal.set(fallback);
				}
			} catch (err) {
				console.error("Failed to get recommended terminal", err);
			}
		}
	}
}

/**
 * Sets the `stackBusy` state while running `fn`, and clears it afterwards.
 * Used to show a busy spinner on commits and block interaction on affected
 * stacks during operations like squash, move, uncommit, etc.
 */
export async function withStackBusy(
	uiState: UiState,
	projectId: string,
	opts: { commitId?: string; stackIds?: string[] },
	fn: () => Promise<void>,
) {
	uiState.project(projectId).stackBusy.set({ commitId: opts.commitId, stackIds: opts.stackIds });
	try {
		await fn();
	} finally {
		uiState.project(projectId).stackBusy.set(undefined);
	}
}
