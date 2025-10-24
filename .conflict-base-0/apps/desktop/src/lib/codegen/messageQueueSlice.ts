import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { ModelType, PermissionMode, ThinkingLevel } from '$lib/codegen/types';

type Message = {
	thinkingLevel: ThinkingLevel;
	model: ModelType;
	permissionMode: PermissionMode;
	prompt: string;
};

export type MessageQueue = {
	projectId: string;
	stackId: string;
	head: string;
	isProcessing: boolean;
	// Ordered from youngest to oldeset
	messages: Message[];
};

export const messageQueueAdapter = createEntityAdapter<MessageQueue, MessageQueue['stackId']>({
	selectId: (a) => a.stackId
});

export const messageQueueSlice = createSlice({
	name: 'messageQueue',
	initialState: messageQueueAdapter.getInitialState(),
	reducers: {
		upsert: messageQueueAdapter.upsertOne,
		remove: messageQueueAdapter.removeOne
	}
});

export const messageQueueSelectors = messageQueueAdapter.getSelectors();
