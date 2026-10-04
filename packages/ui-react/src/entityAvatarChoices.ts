/** The glyph's colours, in the order the picker offers them: the outline first, then the fills. */
export const entityAvatarColours = [
	"outline",
	"pop",
	"safe",
	"purple",
	"danger",
	"warn",
	"gray",
] as const;

export type EntityAvatarColour = (typeof entityAvatarColours)[number];

/** What an entity wears: the glyph in one of its colours, an emoji, or a picture the host supplies. */
export type EntityAvatarValue =
	| { _tag: "Glyph"; colour: EntityAvatarColour }
	| { _tag: "Emoji"; emoji: string }
	| { _tag: "Picture"; src: string };

/** One choice in the picker, and the words it is found by. */
export type EntityAvatarOption = { value: EntityAvatarValue; name: string; keywords?: string };

/**
 * A row of choices in the picker. `small` lays them out seven to a row and scrolls past five rows,
 * for long sets such as emoji; `large` four to a row, for a few pictures worth seeing whole.
 */
export type EntityAvatarSection = {
	name: string;
	options: ReadonlyArray<EntityAvatarOption>;
	size?: "small" | "large";
};

export const sameEntityAvatar = (a: EntityAvatarValue, b: EntityAvatarValue): boolean => {
	switch (a._tag) {
		case "Glyph":
			return b._tag === "Glyph" && a.colour === b.colour;
		case "Emoji":
			return b._tag === "Emoji" && a.emoji === b.emoji;
		case "Picture":
			return b._tag === "Picture" && a.src === b.src;
	}
};

const colourNames: Record<EntityAvatarColour, string> = {
	outline: "Outline",
	pop: "Teal",
	safe: "Green",
	purple: "Purple",
	danger: "Red",
	warn: "Orange",
	gray: "Gray",
};

/** The glyph in each of its colours: what a project picks from first. */
export const entityAvatarGlyphSection: EntityAvatarSection = {
	name: "Colours",
	options: entityAvatarColours.map((colour) => ({
		value: { _tag: "Glyph", colour },
		name: colourNames[colour],
		keywords: "colour color glyph",
	})),
};

/**
 * The emoji an entity can wear as its avatar, in the order the picker lays them out, each with the
 * words it is found by. A short list chosen to tell machines and sessions apart at a glance, not
 * the whole Unicode set: the picker is a grid to pick from, not a search engine.
 */
const entityEmoji: ReadonlyArray<{ emoji: string; name: string; keywords: string }> = [
	{ emoji: "🕹️", name: "joystick", keywords: "game arcade" },
	{ emoji: "🐶", name: "dog", keywords: "puppy pet animal" },
	{ emoji: "🐱", name: "cat", keywords: "kitten pet animal" },
	{ emoji: "🐸", name: "frog", keywords: "animal green" },
	{ emoji: "🐻", name: "bear", keywords: "animal" },
	{ emoji: "🌻", name: "sunflower", keywords: "flower plant yellow" },
	{ emoji: "🐙", name: "octopus", keywords: "animal sea" },
	{ emoji: "🦄", name: "unicorn", keywords: "animal magic" },
	{ emoji: "🐝", name: "bee", keywords: "insect animal honey" },
	{ emoji: "🍕", name: "pizza", keywords: "food" },
	{ emoji: "🌵", name: "cactus", keywords: "plant desert green" },
	{ emoji: "🚀", name: "rocket", keywords: "space launch ship" },
	{ emoji: "🎸", name: "guitar", keywords: "music instrument" },
	{ emoji: "🍩", name: "doughnut", keywords: "donut food sweet" },
	{ emoji: "🌈", name: "rainbow", keywords: "weather colour" },
	{ emoji: "🔥", name: "fire", keywords: "flame hot" },
	{ emoji: "🦖", name: "dinosaur", keywords: "t-rex animal" },
	{ emoji: "🍉", name: "watermelon", keywords: "fruit food" },
	{ emoji: "⚡", name: "lightning", keywords: "bolt zap electric fast" },
	{ emoji: "🐧", name: "penguin", keywords: "animal bird linux" },
	{ emoji: "🍋", name: "lemon", keywords: "fruit food yellow" },
	{ emoji: "💻", name: "laptop", keywords: "computer machine mac" },
	{ emoji: "🖥️", name: "desktop computer", keywords: "machine monitor screen" },
	{ emoji: "📺", name: "television", keywords: "tv screen" },
	{ emoji: "📱", name: "phone", keywords: "mobile device" },
	{ emoji: "⌨️", name: "keyboard", keywords: "computer typing" },
	{ emoji: "🤖", name: "robot", keywords: "bot agent ai machine" },
	{ emoji: "👾", name: "alien monster", keywords: "game space invader" },
	{ emoji: "🛰️", name: "satellite", keywords: "space orbit remote" },
	{ emoji: "☁️", name: "cloud", keywords: "weather remote server" },
	{ emoji: "🌙", name: "moon", keywords: "night crescent" },
	{ emoji: "⭐", name: "star", keywords: "favourite" },
	{ emoji: "🪐", name: "planet", keywords: "saturn space" },
	{ emoji: "🌍", name: "globe", keywords: "earth world planet" },
	{ emoji: "🏔️", name: "mountain", keywords: "snow peak" },
	{ emoji: "🌊", name: "wave", keywords: "sea ocean water" },
	{ emoji: "🍀", name: "clover", keywords: "luck plant green" },
	{ emoji: "🌲", name: "tree", keywords: "evergreen forest plant" },
	{ emoji: "🍄", name: "mushroom", keywords: "plant fungus" },
	{ emoji: "🐢", name: "turtle", keywords: "animal slow" },
	{ emoji: "🦊", name: "fox", keywords: "animal" },
	{ emoji: "🐼", name: "panda", keywords: "animal bear" },
	{ emoji: "🦉", name: "owl", keywords: "animal bird night" },
	{ emoji: "🐳", name: "whale", keywords: "animal sea docker" },
	{ emoji: "🦀", name: "crab", keywords: "animal sea rust" },
	{ emoji: "🐞", name: "ladybug", keywords: "insect bug animal" },
	{ emoji: "🦋", name: "butterfly", keywords: "insect animal" },
	{ emoji: "🍓", name: "strawberry", keywords: "fruit food" },
	{ emoji: "🥑", name: "avocado", keywords: "fruit food green" },
	{ emoji: "🌮", name: "taco", keywords: "food" },
	{ emoji: "☕", name: "coffee", keywords: "drink hot cup" },
	{ emoji: "🍪", name: "cookie", keywords: "food sweet biscuit" },
	{ emoji: "🎮", name: "game controller", keywords: "gamepad play" },
	{ emoji: "🎲", name: "dice", keywords: "game random" },
	{ emoji: "🧩", name: "puzzle", keywords: "piece jigsaw" },
	{ emoji: "🎯", name: "target", keywords: "bullseye dart" },
	{ emoji: "🎨", name: "palette", keywords: "art paint design" },
	{ emoji: "📚", name: "books", keywords: "library read" },
	{ emoji: "🔬", name: "microscope", keywords: "science lab" },
	{ emoji: "🧪", name: "test tube", keywords: "science lab experiment" },
	{ emoji: "🔭", name: "telescope", keywords: "space science" },
	{ emoji: "⚙️", name: "gear", keywords: "settings cog machine" },
	{ emoji: "🔧", name: "wrench", keywords: "tool fix spanner" },
	{ emoji: "🧲", name: "magnet", keywords: "attract" },
	{ emoji: "💡", name: "light bulb", keywords: "idea" },
	{ emoji: "🔋", name: "battery", keywords: "power energy" },
	{ emoji: "📡", name: "antenna", keywords: "satellite dish signal remote" },
	{ emoji: "🗿", name: "moai", keywords: "statue stone" },
	{ emoji: "🏰", name: "castle", keywords: "building" },
	{ emoji: "🚲", name: "bicycle", keywords: "bike" },
	{ emoji: "⛵", name: "sailboat", keywords: "boat sea" },
	{ emoji: "✈️", name: "airplane", keywords: "plane travel flight" },
	{ emoji: "🎈", name: "balloon", keywords: "party" },
	{ emoji: "👻", name: "ghost", keywords: "spooky halloween" },
	{ emoji: "💎", name: "gem", keywords: "diamond jewel" },
	{ emoji: "🔮", name: "crystal ball", keywords: "magic" },
	{ emoji: "🧭", name: "compass", keywords: "direction navigate" },
	{ emoji: "🏆", name: "trophy", keywords: "win prize" },
	{ emoji: "🎹", name: "piano", keywords: "music keyboard instrument" },
	{ emoji: "🎧", name: "headphones", keywords: "music audio" },
	{ emoji: "📷", name: "camera", keywords: "photo picture" },
	{ emoji: "🌋", name: "volcano", keywords: "mountain eruption" },
	{ emoji: "🍔", name: "burger", keywords: "hamburger food" },
	{ emoji: "🐿️", name: "chipmunk", keywords: "squirrel animal" },
];

/** The emoji a project can wear instead of the glyph. */
export const entityAvatarEmojiSection: EntityAvatarSection = {
	name: "Emoji",
	options: entityEmoji.map(({ emoji, name, keywords }) => ({
		value: { _tag: "Emoji", emoji },
		name,
		keywords,
	})),
};

/** The sections with only the options whose name or keywords hold every word typed. */
export const searchEntityAvatarSections = (
	sections: ReadonlyArray<EntityAvatarSection>,
	query: string,
): ReadonlyArray<EntityAvatarSection> => {
	const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
	if (words.length === 0) return sections;
	return sections
		.map((section) => ({
			...section,
			options: section.options.filter(({ name, keywords = "" }) => {
				const haystack = `${name} ${keywords}`.toLowerCase();
				return words.every((word) => haystack.includes(word));
			}),
		}))
		.filter((section) => section.options.length > 0);
};
