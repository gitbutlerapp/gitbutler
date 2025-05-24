import type { ChatMessage } from '@gitbutler/shared/chat/types';

export default class ReplyHandler {
	private _inReplyTo = $state<ChatMessage | undefined>();

	replyTo(message: ChatMessage) {
		this._inReplyTo = message;
	}

	clear() {
		this._inReplyTo = undefined;
	}

	get inReplyTo() {
		return this._inReplyTo;
	}
}
