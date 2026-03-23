import { createEntityAdapter, createSlice } from "@reduxjs/toolkit";
import type { ModelType, PermissionMode, ThinkingLevel } from "$lib/state/uiState.svelte";

type MessageAttachment =
	| { type: "file"; path: string; commitId?: string }
	| { type: "lines"; path: string; start: number; end: number; commitId?: string }
	| { type: "commit"; commitId: string }
	| { type: "directory"; path: string }
	| { type: "folder"; path: string };

type Message = {
	thinkingLevel: ThinkingLevel;
	model: ModelType;
	permissionMode: PermissionMode;
	prompt: string;
	attachments?: MessageAttachment[];
};

export type MessageQueue = {
	projectId: string;
	stackId: string;
	head: string;
	isProcessing: boolean;
	// Ordered from youngest to oldeset
	messages: Message[];
};

export const messageQueueAdapter = createEntityAdapter<MessageQueue, MessageQueue["stackId"]>({
	selectId: (a) => a.stackId,
});

export const messageQueueSlice = createSlice({
	name: "messageQueue",
	initialState: messageQueueAdapter.getInitialState(),
	reducers: {
		upsert: messageQueueAdapter.upsertOne,
		remove: messageQueueAdapter.removeOne,
	},
});

export const messageQueueSelectors = messageQueueAdapter.getSelectors();
