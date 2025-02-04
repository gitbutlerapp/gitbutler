import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
import type { UserMaybe, UserSimple } from '@gitbutler/shared/users/types';

const UNKNOWN_AUTHOR = 'Unknown author';

export type TimestampedEvent = {
	createdAt: string;
	updatedAt: string;
};

function isSameDay(date1: Date, date2: Date): boolean {
	return (
		date1.getFullYear() === date2.getFullYear() &&
		date1.getMonth() === date2.getMonth() &&
		date1.getDate() === date2.getDate()
	);
}

export function eventTimeStamp(event: TimestampedEvent): string {
	const creationDate = new Date(event.createdAt);

	const createdToday = isSameDay(creationDate, new Date());

	if (createdToday) {
		return (
			'Today at ' +
			creationDate.toLocaleTimeString('en-US', {
				hour: 'numeric',
				minute: 'numeric'
			})
		);
	}

	return getTimeAgo(creationDate);
}

export function getMultipleContributorNames(contributors: UserMaybe[]): string {
	if (contributors.length === 0) {
		return UNKNOWN_AUTHOR;
	}

	return contributors
		.map((contributor) => {
			if (contributor.user) {
				const user = contributor.user;
				return user.login ?? user.name ?? user.email ?? UNKNOWN_AUTHOR;
			} else {
				return contributor.email;
			}
		})
		.join(', ');
}

export function embedUserMention(username: string): string {
	return `<<@${username}>>`;
}

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
