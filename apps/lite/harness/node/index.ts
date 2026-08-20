/**
 * The plugin host's side of the transport: one function taking
 * `{ endpoint, params }`, dispatched through the same endpoint table
 * electron main registers on ipcMain. The harness binding adapts its own
 * channel to `handle` and supplies the host-only handlers, checked by the
 * same `HostOnlyKey` contract electron main meets.
 */
import path from "node:path";
import * as sdk from "@gitbutler/but-sdk";
import type { ProjectForFrontend } from "@gitbutler/but-sdk";
import {
	createEndpointTable,
	type Handler,
	type HandlerOverrides,
	type HostOnlyKey,
} from "#electron/endpoint-table.ts";
import type { WatcherSubscribeParams, WatcherUnsubscribeParams } from "#electron/ipc.ts";
import { createHostWatcher, type WatcherEmit } from "./watcher.js";

/** @public */
export type ButIpcRequest = {
	endpoint: string;
	params?: unknown;
};

/** @public */
export type ButIpcHandle = (request: ButIpcRequest) => Promise<unknown>;

/**
 * The watcher lives here, so the binding supplies `emit` instead of watcher
 * handlers. Subscribe/unsubscribe are answered before the table lookup
 * (electron also registers them outside its table); stopAll joins the table
 * as an override.
 * @public
 */
export const createButIpcHandler = ({
	emit,
	hostOverrides,
}: {
	emit: WatcherEmit;
	hostOverrides: HandlerOverrides & { [K in Exclude<HostOnlyKey, "watcherStopAll">]: Handler<K> };
}): ButIpcHandle => {
	const watcher = createHostWatcher(emit);
	const table = new Map(
		createEndpointTable({ watcherStopAll: () => watcher.stopAll(), ...hostOverrides }),
	);

	return ({ endpoint, params }) => {
		if (endpoint === "watcherSubscribe")
			return watcher.subscribe((params as WatcherSubscribeParams).projectId);
		if (endpoint === "watcherUnsubscribe") {
			return Promise.resolve(
				watcher.unsubscribe((params as WatcherUnsubscribeParams).subscriptionId),
			);
		}

		const handler = table.get(endpoint);
		if (!handler) return Promise.reject(new Error(`Unknown endpoint: ${endpoint}`));
		return Promise.resolve(handler(params));
	};
};

/**
 * @public
 * Resolve which project the panel shows. Candidates are tried in order
 * against the registered projects; a candidate inside a project's worktree
 * matches, and the deepest worktree wins when projects nest. With no match,
 * a lone registered project is the answer; otherwise null.
 */
export const resolveProject = async (
	candidates: Array<string>,
): Promise<ProjectForFrontend | null> => {
	const projects = await sdk.listProjectsStateless();

	// `path.relative` rather than a string prefix: it settles separators and
	// `..` on every platform, where a `/` prefix test misses Windows paths.
	const contains = (root: string, candidate: string): boolean => {
		const rel = path.relative(root, candidate);
		return rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel));
	};

	for (const candidate of candidates) {
		const matches = projects
			.filter((p) => contains(p.path, candidate))
			.sort((a, b) => b.path.length - a.path.length);
		if (matches[0]) return matches[0];
	}

	return projects.length === 1 ? (projects[0] ?? null) : null;
};
