import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Subscribe = (listener: (payload: unknown) => void) => () => void;

const toError = (error: unknown): Error => {
	if (error instanceof Error) return error;
	if (typeof error === "object" && error !== null && "message" in error)
		return new Error(String(error.message));
	return new Error(String(error));
};

export const backend = async (
	command: string,
	args?: Record<string, unknown>,
): Promise<unknown> => {
	try {
		return await invoke(command, args);
	} catch (error) {
		throw toError(error);
	}
};

export const listenNow = (event: string, listener: (payload: unknown) => void): (() => void) => {
	const unlisten = listen(event, ({ payload }) => listener(payload));
	return () => void unlisten.then((stop) => stop());
};
