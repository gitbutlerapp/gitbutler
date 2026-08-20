import * as sdk from "@gitbutler/but-sdk";
import { apiParamNames } from "@gitbutler/but-sdk/api-param-names";
import { exposedEndpoints, type Endpoint, type LiteElectronApi, type PayloadFor } from "./ipc.js";

/**
 * Maps endpoint names to SDK calls. Handlers are generated from each call's
 * argument names, so arguments cannot end up in the wrong order. Each host
 * (electron main, the harness host) registers the table on its own channel
 * and injects the few handlers only it can provide.
 */

/** Members the renderer implements itself; they have no host-side handler. */
type RendererOnlyKey = "onAskpassPrompt" | "onDeepLink" | "onFullScreenChange" | "platform";

/** Handlers needing the transport event itself, or taking variadic arguments. */
type ImperativeKey =
	| "isFullScreen"
	| "pathJoin"
	| "showNativeMenu"
	| "streamAiResponse"
	| "watcherSubscribe"
	// The preload wraps the id in an object, so the payload is not the argument.
	| "watcherUnsubscribe";

export type TableKey = Exclude<keyof LiteElectronApi, RendererOnlyKey | ImperativeKey>;

/**
 * Handler types are read off `LiteElectronApi`, so a payload or result that
 * drifts from what the renderer expects is a compile error.
 */
export type Handler<K extends TableKey> = LiteElectronApi[K] extends (params: infer P) => infer R
	? (params: P) => R | Awaited<R>
	: never;

export type HandlerOverrides = { [K in TableKey]?: Handler<K> };

/** A table entry, callable with whatever a host's channel delivered. */
type TableHandler = (params: unknown) => unknown;

/**
 * A derived handler pulls each argument out of the payload by name, so it
 * cannot pass them in the wrong order. A hand-written call can, silently:
 * `commitMoveChangesBetween` takes two commit ids that are both strings.
 */
const derivedHandler =
	(key: Endpoint) =>
	(params: unknown): unknown => {
		const names: ReadonlyArray<string> = apiParamNames[key];
		const call = sdk[key] as (...args: Array<unknown>) => unknown;
		// A lone argument is sent as itself; the rest arrive as a payload.
		return names.length === 1
			? call(params)
			: call(...names.map((name) => (params as Record<string, unknown>)[name]));
	};

type DerivedKey = Extract<TableKey, Endpoint>;

/**
 * Members only a host can answer. Hosts check their overrides against this
 * type, so forgetting one is a compile error.
 */
export type HostOnlyKey = Exclude<TableKey, Endpoint>;

type PayloadOf<K extends TableKey> = Parameters<LiteElectronApi[K]>[0];

/**
 * Compile-time check that every derived handler can supply its call's
 * arguments: multi-argument calls need a payload carrying every name,
 * single-argument calls take the payload itself. Anything else must be
 * an override.
 */
type CannotSupplyItsArguments = {
	[K in DerivedKey]: (typeof apiParamNames)[K]["length"] extends 0
		? never
		: (typeof apiParamNames)[K]["length"] extends 1
			? PayloadOf<K> extends Parameters<(typeof sdk)[K]>[0]
				? never
				: K
			: PayloadOf<K> extends PayloadFor<K>
				? never
				: K;
}[DerivedKey];
type AssertNever<T extends never> = T;
type _EveryDerivedHandlerCanSupplyItsArguments = AssertNever<CannotSupplyItsArguments>;

/**
 * Builds the full table: a derived handler for every exposed endpoint not
 * overridden, then the overrides. Overrides register even when nothing was
 * derived for them — that is how host-only members get in.
 */
export const createEndpointTable = (
	hostOverrides: HandlerOverrides,
): ReadonlyArray<[string, TableHandler]> => {
	const overrides: HandlerOverrides = hostOverrides;
	const table: Array<[string, TableHandler]> = [];

	for (const key of exposedEndpoints) {
		if (key in overrides) continue;
		table.push([key, derivedHandler(key)]);
	}
	// The one unchecked step, kept here so hosts can call the table directly:
	// each override really does take its own payload type, and only the wire
	// data a host passes can say whether that is what arrived.
	for (const [name, handler] of Object.entries(overrides))
		table.push([name, handler as TableHandler]);

	return table;
};
