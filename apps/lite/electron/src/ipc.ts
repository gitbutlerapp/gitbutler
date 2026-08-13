import type {
	AiConfiguration,
	AiConfigurationUpdate,
	WatcherEvent,
	AskpassPromptEvent,
} from "@gitbutler/but-sdk";
import type * as sdk from "@gitbutler/but-sdk";
import { apiParamNames } from "@gitbutler/but-sdk/api-param-names";
import type { GUISettings } from "./settings.js";

/**
 * What the renderer can call: every SDK endpoint, whose signatures are the
 * SDK's, plus the members electron implements itself.
 */
export type LiteElectronApi = {
	[K in Endpoint]: EndpointFn<K>;
} & {
	onAskpassPrompt: (callback: (event: AskpassPromptEvent) => void) => () => void;
	askpassSubmitPromptResponse: (params: AskpassSubmitPromptResponseParams) => Promise<void>;
	clipboardWriteText: (text: string) => Promise<void>;
	getAiConfiguration: () => Promise<AiConfiguration>;
	getVersion: () => Promise<string>;
	isFullScreen: () => Promise<boolean>;
	onFullScreenChange: (callback: (fullScreen: boolean) => void) => () => void;
	openInWebBrowser: (url: string) => Promise<void>;
	pathJoin: (...paths: Array<string>) => Promise<string>;
	resetAiConfiguration: () => Promise<AiConfiguration>;
	showNativeMenu: (params: ShowNativeMenuParams) => Promise<string | null>;
	streamAiResponse: (
		systemMessage: string,
		prompt: string,
		onToken: (token: string) => void,
	) => Promise<string>;
	updateAiConfiguration: (update: AiConfigurationUpdate) => Promise<AiConfiguration>;
	watcherSubscribe: (projectId: string, callback: (event: WatcherEvent) => void) => Promise<string>;
	watcherUnsubscribe: (subscriptionId: string) => Promise<boolean>;
	watcherStopAll: () => Promise<number>;
	readGUISettings: () => Promise<GUISettings>;
	writeGUISettings: (settings: GUISettings) => Promise<void>;
	platform: string;
};

/**
 * The SDK endpoints the renderer can call: all of them, each under its own
 * name as the IPC channel, so a new declaration in Rust reaches `window.lite`
 * with nothing to keep in step.
 */
// `Object.keys` erases key types; the record's keys are exactly these.
export const exposedEndpoints = Object.keys(apiParamNames) as ReadonlyArray<Endpoint>;

/** Members the main process answers itself rather than forwarding to the SDK. */
export const localEndpoints = [
	"askpassPrompt",
	"askpassSubmitPromptResponse",
	"clipboardWriteText",
	"fullScreenChange",
	"getAiConfiguration",
	"getVersion",
	"isFullScreen",
	"openInWebBrowser",
	"pathJoin",
	"readGUISettings",
	"resetAiConfiguration",
	"showNativeMenu",
	"streamAiResponse",
	"updateAiConfiguration",
	"watcherStopAll",
	"watcherSubscribe",
	"watcherUnsubscribe",
	"writeGUISettings",
] as const;

/** An endpoint the SDK exposes to JavaScript. */
export type Endpoint = keyof typeof apiParamNames & keyof typeof sdk;

/**
 * The payload for an endpoint, named.
 *
 * napi-rs declares endpoints positionally; the SDK separately emits each
 * one's parameter names, generated from the Rust signature that owns them.
 * Pairing the two gives the object shape without restating a single type —
 * every type here still comes from the declaration.
 */
export type PayloadFor<K extends Endpoint> = {
	// Indices only, never the array's own members: mapping straight over
	// `keyof tuple` is homomorphic and would yield an array-like type.
	[I in Extract<keyof (typeof apiParamNames)[K], `${number}`> as (typeof apiParamNames)[K][I] &
		string]: (typeof sdk)[K] extends (...args: infer A) => unknown
		? I extends keyof A
			? A[I]
			: never
		: never;
};

type Result<K extends Endpoint> = ReturnType<(typeof sdk)[K]>;

/** Takes nothing, the lone argument itself, or a payload — by parameter count. */
type EndpointFn<K extends Endpoint> = (typeof apiParamNames)[K]["length"] extends 0
	? () => Result<K>
	: (typeof apiParamNames)[K]["length"] extends 1
		? (arg: Parameters<(typeof sdk)[K]>[0]) => Result<K>
		: (params: PayloadFor<K>) => Result<K>;

// Shapes electron owns: no SDK declaration behind them, so they are written
// out by hand — the only types in this file that are.

/** Askpass is not a `#[but_api]` endpoint, so there are no names to derive from. */
export interface AskpassSubmitPromptResponseParams {
	id: string;
	response: string | null;
}

/** Watcher wire shapes; the preload wraps these behind a callback API. */
export interface WatcherSubscribeParams {
	projectId: string;
}

export interface WatcherSubscribeResult {
	subscriptionId: string;
	eventChannel: string;
}

export interface WatcherUnsubscribeParams {
	subscriptionId: string;
}

export interface StreamAiResponseParams {
	requestId: string;
	systemMessage: string;
	prompt: string;
}

export interface StreamAiResponseToken {
	requestId: string;
	token: string;
}

export interface NativeMenuPosition {
	x: number;
	y: number;
}

type NativeMenuPopupItemData = {
	label: string;
	accelerator?: string;
	/** Renders the item as a checkbox in the given state. */
	checked?: boolean;
	enabled?: boolean;
	itemId?: string;
	submenu?: Array<NativeMenuPopupItem>;
};

export type NativeMenuPopupItem =
	| { _tag: "Separator" }
	| ({ _tag: "Item" } & NativeMenuPopupItemData);

export interface ShowNativeMenuParams {
	items: Array<NativeMenuPopupItem>;
	position: NativeMenuPosition;
}
