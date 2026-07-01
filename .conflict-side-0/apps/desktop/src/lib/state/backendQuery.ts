import { isBackend, type IBackend } from "$lib/backend";
import { IpcError, isNormalizedError, type NormalizedError } from "$lib/error/normalizedError";
import { isErrorlike } from "@gitbutler/ui/utils/typeguards";
import { type BaseQueryApi, type QueryReturnValue } from "@reduxjs/toolkit/query";
import type { ExtraOptions } from "$lib/state/butlerModule";

import type { BaseQueryFn } from "@reduxjs/toolkit/query";

export type TauriExtraOptions = ExtraOptions & { command?: string };
export type TauriBaseQueryFn = BaseQueryFn<ApiArgs, unknown, unknown, TauriExtraOptions>;

// eslint-disable-next-line func-style
export const tauriBaseQuery: TauriBaseQueryFn = async (
	args: ApiArgs,
	api: BaseQueryApi,
	extra: TauriExtraOptions,
): Promise<QueryReturnValue<unknown, NormalizedError, undefined>> => {
	const command = extra.command;
	if (!command) {
		return newError("Expected a command!");
	}

	if (!hasBackendExtra(api.extra)) {
		return newError("Redux dependency Tauri not found!");
	}

	try {
		const result = { data: await api.extra.backend.invoke(command, args) };
		return result;
	} catch (error: unknown) {
		const name = `API error: (${command})`;
		if (isNormalizedError(error)) {
			// Forward the IpcError fingerprint across the plain-object hop so
			// `applyIpcFingerprint` (Sentry `beforeSend`) can still find it
			// downstream of `normalizedErrorToException` (in `normalizedError.ts`).
			const fingerprint = error instanceof IpcError ? error.fingerprint : undefined;
			return {
				error: { origin: "ipc", name, message: error.message, code: error.code, fingerprint },
			};
		}

		if (isErrorlike(error)) {
			return { error: { origin: "ipc", name, message: error.message } };
		}

		return { error: { origin: "ipc", name, message: String(error) } };
	}
};

function newError(message: string): { error: NormalizedError } {
	return {
		error: { origin: "ipc", name: "Failed to execute Tauri query", message },
	};
}

type ApiArgs = Record<string, unknown> | undefined;

/**
 * Typeguard for accessing injected Tauri dependency safely.
 */
export function hasBackendExtra(extra: unknown): extra is {
	backend: IBackend;
} {
	return (
		!!extra &&
		typeof extra === "object" &&
		extra !== null &&
		"backend" in extra &&
		isBackend(extra.backend)
	);
}
