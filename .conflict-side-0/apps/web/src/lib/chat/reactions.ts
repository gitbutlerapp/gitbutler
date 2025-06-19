import type { User } from '$lib/user/userService';
import type { ChatMessageReaction } from '@gitbutler/shared/chat/types';
import type { EmojiInfo } from '@gitbutler/ui/emoji/utils';

function getUpdatedReactions(
	user: User,
	emoji: EmojiInfo,
	reactions: ChatMessageReaction[]
): ChatMessageReaction[] {
	const login = user.login;
	const hasReaction = reactions.some(
		(reaction) =>
			reaction.reaction === emoji.unicode &&
			reaction.users.some((user) => !!user.login && user.login === login)
	);

	if (hasReaction) {
		// Remove reaction
		return reactions.map((reaction) => {
			if (reaction.reaction === emoji.unicode) {
				reaction.users = reaction.users.filter((user) => !!user.login && user.login !== login);
			}
			return reaction;
		});
	}

	// Add reaction
	const existingUnicode = reactions.find((reaction) => reaction.reaction === emoji.unicode);

	if (existingUnicode) {
		return reactions.map((reaction) => {
			if (reaction.reaction === emoji.unicode) {
				reaction.users.push({
					id: user.id,
					login: login,
					avatarUrl: user.avatar_url,
					email: user.email,
					name: user.name
				});
			}
			return reaction;
		});
	}

	reactions.push({
		reaction: emoji.unicode,
		users: [
			{
				id: user.id,
				login: login,
				avatarUrl: user.avatar_url,
				email: user.email,
				name: user.name
			}
		]
	});

	return reactions;
}

export function updateReactions(
	user: User,
	emoji: EmojiInfo,
	reactions: ChatMessageReaction[]
): ChatMessageReaction[] {
	return getUpdatedReactions(user, emoji, reactions).filter(
		(reaction) => reaction.users.length > 0
	);
}
