import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { LoadableChatChannel } from '$lib/chat/types';

const chatChannelAdapter = createEntityAdapter<LoadableChatChannel, LoadableChatChannel['id']>({
	selectId: (chatChannel) => chatChannel.id
});

const chatChannelsSlice = createSlice({
	name: 'chatChannels',
	initialState: chatChannelAdapter.getInitialState(),
	reducers: {
		addChatChannel: chatChannelAdapter.addOne,
		addChatChannels: chatChannelAdapter.addMany,
		removeChatChannel: chatChannelAdapter.removeOne,
		removeChatChannels: chatChannelAdapter.removeMany,
		upsertChatChannel: loadableUpsert(chatChannelAdapter),
		upsertChatChannels: loadableUpsertMany(chatChannelAdapter)
	}
});

export const chatChannelsReducer = chatChannelsSlice.reducer;

export const chatChannelsSelectors = chatChannelAdapter.getSelectors();
export const {
	addChatChannel,
	addChatChannels,
	removeChatChannel,
	removeChatChannels,
	upsertChatChannel,
	upsertChatChannels
} = chatChannelsSlice.actions;
