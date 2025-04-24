import type { UserSimple } from '@gitbutler/shared/users/types';

export interface MentionMatch {
	user: UserSimple;
	prefix: MentionMatch | string;
	suffix: MentionMatch | string;
}

export function isMentionMatch(value: any): value is MentionMatch {
	return typeof value === 'object' && value !== null && 'id' in value;
}

export interface TextWord {
	type: 'text';
	value: string;
}

export interface MentionWord {
	type: 'mention';
	mention: MentionMatch;
}

export type Word = TextWord | MentionWord;

function getChateMessageMentionMatch(
	word: string,
	userMap: Map<number, UserSimple>
): MentionMatch | undefined {
	if (!word) {
		return undefined;
	}

	const match = word.match(/(.*)<<@(\d+)>>(.*)/);
	if (match) {
		const id = parseInt(match[2]);
		const user = userMap.get(id);
		if (!user) {
			return undefined;
		}

		const prefix = getChateMessageMentionMatch(match[1], userMap) ?? match[1];
		const suffix = getChateMessageMentionMatch(match[3], userMap) ?? match[3];

		return {
			user,
			prefix,
			suffix
		};
	}
	return undefined;
}

export function getChatMessageWords(text: string, userMap: Map<number, UserSimple>): Word[] {
	const words: Word[] = [];
	const listedText = text.split(' ');
	for (const word of listedText) {
		const mention = getChateMessageMentionMatch(word, userMap);

		if (mention) {
			words.push({ type: 'mention', mention });
			continue;
		}

		words.push({ type: 'text', value: word });
	}
	return words;
}
