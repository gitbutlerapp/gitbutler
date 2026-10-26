import styles from "./EntityAvatar.module.css";
import { classes } from "./classes.ts";
import {
	sameEntityAvatar,
	searchEntityAvatarSections,
	type EntityAvatarColour,
	type EntityAvatarSection,
	type EntityAvatarValue,
} from "./entityAvatarChoices.ts";
import { Dropdown, PopupEmpty, PopupSearch, PopupSection } from "./Popup.tsx";
import { ScrollArea } from "./ScrollArea.tsx";
import { useState, type ComponentProps, type FC } from "react";

/** `regular` is 38px, an avatar on its own; `title` a 16px glyph on a 24px ground, beside a title. */
export type EntityAvatarSize = "title" | "regular";

/**
 * The repository glyph an entity wears, as ⚛️ Core draws it: the `repo` icon's outline in the text
 * colour, or a book in one of six hues, its cover filled and edged in a deeper shade over a cream
 * page, the same in light and dark. Its edges keep the icons' 1.5px line at any size. `size` sets it in pixels where it stands on its own, as in a
 * sidebar row; inside an `EntityAvatar` the avatar's size sets it.
 * @import import { EntityGlyph } from "@gitbutler/ui-react/EntityAvatar.tsx";
 */
export const EntityGlyph: FC<{ colour: EntityAvatarColour; size?: number }> = ({
	colour,
	size,
}) => (
	<svg
		className={classes(styles.glyph, styles[colour])}
		viewBox="0 0 16 16"
		fill="none"
		aria-hidden
		{...(size !== undefined ? { width: size, height: size } : undefined)}
	>
		{colour !== "outline" && (
			<path className={styles.cover} d="M14 1.5H4C2.89543 1.5 2 2.39543 2 3.5V12.5H14V1.5Z" />
		)}
		<path className={styles.edge} d="M14 12.5V1.5H4C2.89543 1.5 2 2.39543 2 3.5V12.5" />
		<path
			className={classes(styles.page, styles.edge)}
			d="M14 10.5H4C2.89543 10.5 2 11.3954 2 12.5C2 13.6046 2.89543 14.5 4 14.5H14V10.5Z"
		/>
		<path className={styles.windows} d="M4 3.5H6V4.5H4ZM4 5.5H6V6.5H4ZM4 7.5H6V8.5H4Z" />
	</svg>
);

/**
 * What an entity — a project, a machine, a cloud session — wears in place of a photo: the
 * repository glyph in one of seven colours, an emoji, or a picture the host supplies (a machine's
 * kind). `regular` is 38px, for the avatar slot of `ViewHeader`, a page of its own or the
 * picker's choice of a machine's pictures. `title` is a 16px glyph or emoji on a 24px ground, where
 * a `ViewHeader`'s title has its icon (a repo's own), and the picker's emoji and colours.
 *
 * A picture of the thing, not a control: to let someone change it, use `EntityAvatarPicker`.
 * @import import { EntityAvatar } from "@gitbutler/ui-react/EntityAvatar.tsx";
 */
export const EntityAvatar: FC<
	{ value: EntityAvatarValue; size?: EntityAvatarSize } & ComponentProps<"span">
> = ({ value, size = "regular", ...props }) => (
	<span {...props} className={classes(props.className, styles.avatar, styles[size])}>
		{value._tag === "Emoji" ? (
			<span className={styles.emoji}>{value.emoji}</span>
		) : value._tag === "Picture" ? (
			<img className={styles.picture} src={value.src} alt="" draggable={false} />
		) : (
			<EntityGlyph colour={value.colour} />
		)}
	</span>
);

/**
 * An entity's avatar that changes when pressed. The host says what there is to pick from, in
 * sections: a project's are `entityAvatarGlyphSection` and `entityAvatarEmojiSection`, with search;
 * a machine's are its kinds as pictures, too few to need it. Picking one closes the picker; where
 * the choice is kept is the host's.
 * @import import { EntityAvatarPicker } from "@gitbutler/ui-react/EntityAvatar.tsx";
 */
export const EntityAvatarPicker: FC<{
	value: EntityAvatarValue;
	onChange: (value: EntityAvatarValue) => void;
	/** Names what the avatar belongs to, for the button: "Change pave--macbook-pro's avatar". */
	label: string;
	sections: ReadonlyArray<EntityAvatarSection>;
	/** The search field's placeholder, "Search emoji…". Omit for a set short enough to scan. */
	searchPlaceholder?: string;
	/** The avatar the picker opens from: `title` where a header's title has its icon. */
	size?: EntityAvatarSize;
	/** For the button the picker opens from, as a host's margin that hangs its ground past the glyph. */
	className?: string;
}> = ({ value, onChange, label, sections, searchPlaceholder, size = "regular", className }) => {
	const [open, setOpen] = useState(false);
	const [query, setQuery] = useState("");
	const shown = searchEntityAvatarSections(sections, query);

	const openChange = (next: boolean) => {
		setOpen(next);
		if (!next) setQuery("");
	};

	return (
		<Dropdown
			open={open}
			onOpenChange={openChange}
			trigger={
				<button
					type="button"
					aria-label={label}
					className={classes(className, styles.trigger, size === "title" && styles.titleTrigger)}
				>
					<EntityAvatar value={value} size={size} />
				</button>
			}
		>
			{searchPlaceholder !== undefined && (
				<PopupSearch
					aria-label={searchPlaceholder.replace(/…$/, "")}
					placeholder={searchPlaceholder}
					value={query}
					onChange={(event) => setQuery(event.target.value)}
					onClear={query === "" ? undefined : () => setQuery("")}
				/>
			)}
			{shown.length === 0 ? (
				<PopupEmpty
					query={query}
					nothingFound="Nothing found"
					nothingToList="Nothing to pick"
					onClearSearch={() => setQuery("")}
				/>
			) : (
				shown.map(({ name, options, size: optionSize = "title" }) => (
					<PopupSection key={name}>
						<ScrollArea className={optionSize === "title" ? styles.titleArea : undefined}>
							<fieldset aria-label={name} className={styles[`${optionSize}Options`]}>
								{options.map((option) => (
									<button
										key={option.name}
										type="button"
										aria-label={option.name}
										aria-pressed={sameEntityAvatar(value, option.value)}
										className={styles.option}
										onClick={() => {
											onChange(option.value);
											openChange(false);
										}}
									>
										<EntityAvatar value={option.value} size={optionSize} />
									</button>
								))}
							</fieldset>
						</ScrollArea>
					</PopupSection>
				))
			)}
		</Dropdown>
	);
};
