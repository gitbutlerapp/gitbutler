import { buildLoadableTable } from '$lib/redux/defaultSlices';
import type { LoadableChatChannel } from '$lib/chat/types';

export const chatChannelTable = buildLoadableTable<LoadableChatChannel>('chatChannel');
