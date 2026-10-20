import { Channel } from "@tauri-apps/api/core";
import type { StreamAiResponseParams, StreamAiResponseToken } from "#electron/ipc.ts";
import { backend, type Subscribe } from "./backend.ts";

const tokenListeners = new Set<(payload: StreamAiResponseToken) => void>();

export const handlers = {
	streamAiResponse: ({ requestId, systemMessage, prompt }: StreamAiResponseParams) => {
		const onToken = new Channel<string>((token) => {
			for (const listener of tokenListeners) listener({ requestId, token });
		});
		return backend("stream_ai_response", { systemMessage, prompt, onToken }) as Promise<string>;
	},
};

export const channels: Record<string, Subscribe> = {
	streamAiResponseToken: (listener) => {
		tokenListeners.add(listener);
		return () => tokenListeners.delete(listener);
	},
};
