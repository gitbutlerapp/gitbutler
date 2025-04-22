import { createEntityAdapter } from '@reduxjs/toolkit';
import type { IrcLog } from '$lib/irc/types';

export const logsAdapter = createEntityAdapter<IrcLog, string>({
	selectId: (model) => model.msgid || String(model.timestamp)
});

export const logSelectors = logsAdapter.getSelectors();
