import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';

export type MessageQueue = {
	stackId: string;
	projectId: string;
	// Ordered from youngest to oldeset
	messages: string[];
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

export const { selectAll: selectAllMessageQueues } = messageQueueAdapter.getSelectors();
