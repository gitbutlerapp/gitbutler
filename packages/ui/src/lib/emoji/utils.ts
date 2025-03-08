import emojiData from 'emojibase-data/en/compact.json';
import emojiByHexcode from 'emojibase-data/en/shortcodes/github.json';

export const EMOJI_SHORTCODE_REGEX = /(^|\s):([0-9a-z+_-]+):($|\s)/;
export const EMOJI_SHORTCODE_SEARCH_REGEX = /(^|\s):([0-9a-z+_-]*)$/;
export const LS_RECENT_EMOJIS_KEY = 'recent-emojis';

export const DEFAULT_EMOJI: EmojiInfo[] = [
	{ label: 'like', unicode: '👍' },
	{ label: 'dislike', unicode: '👎' },
	{ label: 'celebrate', unicode: '🎉' },
	{ label: 'heart', unicode: '❤️' },
	{ label: 'rocket', unicode: '🚀' },
	{ label: 'poopsie', unicode: '💩' },
	{ label: 'butreq', unicode: '🍑' },
	{ label: 'happysmile', unicode: '🙂' },
	{ label: 'veryhappysmile', unicode: '😀' },
	{ label: 'unhappysmile', unicode: '🙁' },
	{ label: 'tongue', unicode: '😛' },
	{ label: 'evil', unicode: '😈' },
	{ label: 'aubergine', unicode: '🍆' },
	{ label: 'surprised', unicode: '😲' },
	{ label: 'neutral', unicode: '😐' },
	{ label: 'cheeky', unicode: '😜' },
	{ label: 'unsure', unicode: '😕' }
];

export type EmojiInfo = {
	unicode: string;
	label: string;
	shortcodes?: string[] | string;
};

function isEmojiInfo(something: unknown): something is EmojiInfo {
	return (
		typeof something === 'object' &&
		something !== null &&
		typeof (something as EmojiInfo).unicode === 'string' &&
		typeof (something as EmojiInfo).label === 'string'
	);
}

export type RecentlyUsedEmojiInfo = EmojiInfo & {
	timesUsed: number;
	lastUsed: string; // Date string
};

function isRecentlyUsedEmojiInfo(something: unknown): something is RecentlyUsedEmojiInfo {
	if (!isEmojiInfo(something)) {
		return false;
	}
	return (
		typeof (something as RecentlyUsedEmojiInfo).timesUsed === 'number' &&
		typeof (something as RecentlyUsedEmojiInfo).lastUsed === 'string'
	);
}

function isRecentlyUsedEmojiInfos(something: unknown): something is RecentlyUsedEmojiInfo[] {
	if (!Array.isArray(something)) {
		return false;
	}

	return something.every(isRecentlyUsedEmojiInfo);
}

export function getRecentEmojis(): RecentlyUsedEmojiInfo[] | undefined {
	const recentEmojis = localStorage.getItem(LS_RECENT_EMOJIS_KEY);
	if (!recentEmojis) {
		return undefined;
	}

	try {
		const parsed = JSON.parse(recentEmojis) as unknown;
		if (!isRecentlyUsedEmojiInfos(parsed)) {
			console.error('Invalid recent emojis data:', parsed);
			return undefined;
		}

		return parsed.sort((a, b) => {
			const dateA = new Date(a.lastUsed);
			const dateB = new Date(b.lastUsed);
			const timesUsedDiff = b.timesUsed - a.timesUsed;
			return timesUsedDiff === 0 ? dateB.getTime() - dateA.getTime() : timesUsedDiff;
		});
	} catch {
		return undefined;
	}
}

function writeRecentEmojis(emojis: RecentlyUsedEmojiInfo[]) {
	const serialized = JSON.stringify(emojis);
	localStorage.setItem(LS_RECENT_EMOJIS_KEY, serialized);
}

export function markRecentlyUsedEmoji(emoji: EmojiInfo): void {
	const recentEmojis = getRecentEmojis() ?? [];
	const existingEmojiIndex = recentEmojis.findIndex((e) => e.unicode === emoji.unicode);

	const recentlyUsedEmoji: RecentlyUsedEmojiInfo = {
		...emoji,
		timesUsed: 1,
		lastUsed: new Date().toISOString()
	};

	let newRecentEmojis = recentEmojis;

	emojiInsertion: {
		if (existingEmojiIndex !== -1) {
			newRecentEmojis[existingEmojiIndex] = {
				...recentlyUsedEmoji,
				timesUsed: recentEmojis[existingEmojiIndex].timesUsed + 1
			};
			break emojiInsertion;
		}

		newRecentEmojis = [
			recentlyUsedEmoji,
			...recentEmojis.filter((e) => e.unicode !== emoji.unicode)
		];
	}

	writeRecentEmojis(newRecentEmojis);
}

export function getInitialEmojis(): EmojiInfo[] {
	const recentEmojis = getRecentEmojis() ?? [];
	return deduplicateEmojis([...recentEmojis, ...DEFAULT_EMOJI]);
}

function deduplicateEmojis(emojis: EmojiInfo[]): EmojiInfo[] {
	const seen = new Set<string>();

	return emojis.filter((emoji) => {
		if (seen.has(emoji.unicode)) {
			return false;
		}
		seen.add(emoji.unicode);
		return true;
	});
}

/**
 * Returns a list of emojis that match the given search query.
 */
export function searchThroughEmojis(searchQuery: string): EmojiInfo[] {
	const initialEmojis = getInitialEmojis();

	if (!searchQuery) return initialEmojis;

	const emojiEntries = Object.entries(emojiByHexcode);
	const emojiHexcodes = emojiEntries
		.filter(([_, shortCodes]) => {
			if (Array.isArray(shortCodes)) {
				return shortCodes.some((shortCode) => shortCode.startsWith(searchQuery));
			}
			return shortCodes.startsWith(searchQuery);
		})
		.map(([hexcode]) => hexcode);

	const matchingData = emojiData.filter(
		(emoji) => emojiHexcodes.includes(emoji.hexcode) || emoji.label.includes(searchQuery)
	);

	const matchingInitial = initialEmojis.filter((emoji) => emoji.label.includes(searchQuery));

	return deduplicateEmojis([...matchingInitial, ...matchingData]);
}

/**
 * Returns the emoji that matches the given shortcode without colons.
 */
export function findEmojiByShortcode(shortcode: string): EmojiInfo | undefined {
	const emoji = Object.entries(emojiByHexcode).find(([_, shortCodes]) => {
		if (Array.isArray(shortCodes)) {
			return shortCodes.includes(shortcode);
		}
		return shortCodes === shortcode;
	});

	if (!emoji) {
		return undefined;
	}

	const compactEmoji = emojiData.find((e) => e.hexcode === emoji[0]);
	return compactEmoji;
}

/**
 * Returns the emoji that matches the given unicode.ß
 */
export function findEmojiByUnicode(unicode: string): EmojiInfo | undefined {
	const initialEmojis = getInitialEmojis();
	const found = initialEmojis.find((emoji) => emoji.unicode === unicode);
	if (found) {
		return found;
	}
	return emojiData.find((emoji) => emoji.unicode === unicode);
}
