import type { Subscribe } from "./backend.ts";

export const handlers = {
	streamAiResponse: () => {
		throw new Error("Streaming AI responses is not available in Tauri yet");
	},
};

export const channels: Record<string, Subscribe> = {};
