import { type DiffLineSelected, type DiffSelection } from '$lib/diff/lineSelection.svelte';
import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
import { extractUserMention } from '@gitbutler/ui/richText/node/mention';
import { encodeDiffLineRange } from '@gitbutler/ui/utils/diffParsing';

export interface SendParams {
	issue?: boolean;
	diffSelection?: DiffSelection;
}

export default class MessageHandler {
	private chatChannelService: ChatChannelsService | undefined;
	private projectId: string | undefined;
	private branchId: string | undefined;
	private changeId: string | undefined;

	private _message = $state<string>();

	init(
		chatChannelService: ChatChannelsService,
		projectId: string,
		branchId: string,
		changeId: string
	) {
		this.chatChannelService = chatChannelService;
		this.projectId = projectId;
		this.branchId = branchId;
		if (changeId !== this.changeId) {
			this._message = undefined;
		}
		this.changeId = changeId;
	}

	private getSendableLine(messageLine: string): [string, string] {
		const messageBuffer: string[] = [];
		const displayBuffer: string[] = [];

		const listedText = messageLine.split(' ');
		for (const word of listedText) {
			const match = extractUserMention(word);

			if (match === null) {
				messageBuffer.push(word);
				displayBuffer.push(word);
				continue;
			}

			const [id, label] = match;
			messageBuffer.push(`<<@${id}>>`);
			displayBuffer.push(label);
		}

		return [messageBuffer.join(' '), displayBuffer.join(' ')];
	}

	private getSendableMessages(): [string, string] | undefined {
		const message = this._message;

		if (!message || message.trim() === '') {
			return undefined;
		}

		const messageBuffer: string[] = [];
		const displayBuffer: string[] = [];

		const messageLines = message.split('\n');
		for (const line of messageLines) {
			const [messageLine, displayLine] = this.getSendableLine(line);
			messageBuffer.push(messageLine);
			displayBuffer.push(displayLine);
		}

		return [messageBuffer.join('\n'), displayBuffer.join('\n')];
	}

	private getDiffRange(lines: DiffLineSelected[] | undefined) {
		if (!lines) {
			return undefined;
		}

		const sortedLines = lines.sort((a, b) => a.index - b.index);
		return encodeDiffLineRange(sortedLines);
	}

	async send(params: SendParams) {
		const messages = this.getSendableMessages();

		if (
			!messages ||
			!this.chatChannelService ||
			!this.projectId ||
			!this.branchId ||
			!this.changeId
		) {
			return;
		}

		const [message, displayMessage] = messages;

		await this.chatChannelService.sendChatMessage({
			projectId: this.projectId,
			branchId: this.branchId,
			changeId: this.changeId,
			chat: message,
			displayableText: displayMessage,
			issue: params.issue,
			diffPath: params.diffSelection?.fileName,
			diffSha: params.diffSelection?.diffSha,
			range: this.getDiffRange(params.diffSelection?.lines)
		});
	}

	update(text: string) {
		this._message = text;
	}

	get message() {
		return this._message;
	}
}
