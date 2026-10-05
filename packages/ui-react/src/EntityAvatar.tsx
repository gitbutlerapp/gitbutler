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
import { useId, useState, type ComponentProps, type FC } from "react";

const Glyph: FC<{ colour: EntityAvatarColour }> = ({ colour }) => {
	// Many avatars can share a page, so each gradient needs an id of its own.
	const gradientId = useId();

	return (
		<svg className={styles.glyph} viewBox="0 0 26 28" fill="none" aria-hidden>
			{colour === "outline" ? (
				<>
					<path
						d="M0.75 5.75C0.75 2.98858 2.98858 0.75 5.75 0.75H24.75V26.75H5.75C2.98858 26.75 0.75 24.5114 0.75 21.75V5.75Z"
						stroke="currentColor"
						strokeWidth="1.5"
						vectorEffect="non-scaling-stroke"
					/>
					<rect x="5.75" y="6.75" width="3" height="14" rx="1.5" fill="currentColor" />
				</>
			) : (
				<>
					<path
						transform="translate(1 1)"
						fillRule="evenodd"
						d="M24 26H5C2.23858 26 0 23.7614 0 21V5C0 2.23858 2.23858 0 5 0H24V26ZM6.5 6C5.67157 6 5 6.67157 5 7.5V18.5C5 19.3284 5.67157 20 6.5 20C7.32843 20 8 19.3284 8 18.5V7.5C8 6.67157 7.32843 6 6.5 6Z"
						fill={`url(#${gradientId})`}
					/>
					<defs>
						<linearGradient
							id={gradientId}
							className={styles[colour]}
							x1="12"
							y1="2"
							x2="12"
							y2="23.26"
							gradientUnits="userSpaceOnUse"
						>
							<stop className={styles.top} />
							<stop offset="1" className={styles.bottom} />
						</linearGradient>
					</defs>
				</>
			)}
		</svg>
	);
};

/**
 * What an entity — a project, a machine, a cloud session — wears in place of a photo: the
 * repository glyph in one of seven colours, an emoji, or a picture the host supplies (a machine's
 * kind). An avatar is 38px wherever it stands, the avatar slot of `ViewHeader`, a page of its own
 * or the picker's choice of a machine's pictures; leave `size` at `regular`. `small` is the picker's
 * emoji and colours, at 28px.
 *
 * A picture of the thing, not a control: to let someone change it, use `EntityAvatarPicker`.
 * @import import { EntityAvatar } from "@gitbutler/ui-react/EntityAvatar.tsx";
 */
export const EntityAvatar: FC<
	{ value: EntityAvatarValue; size?: "small" | "regular" } & ComponentProps<"span">
> = ({ value, size = "regular", ...props }) => (
	<span {...props} className={classes(props.className, styles.avatar, styles[size])}>
		{value._tag === "Emoji" ? (
			<span className={styles.emoji}>{value.emoji}</span>
		) : value._tag === "Picture" ? (
			<img className={styles.picture} src={value.src} alt="" draggable={false} />
		) : (
			<Glyph colour={value.colour} />
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
}> = ({ value, onChange, label, sections, searchPlaceholder }) => {
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
				<button type="button" aria-label={label} className={styles.trigger}>
					<EntityAvatar value={value} />
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
				<PopupEmpty query={query} nothingFound="Nothing found" nothingToList="Nothing to pick" />
			) : (
				shown.map(({ name, options, size: optionSize = "small" }) => (
					<PopupSection key={name}>
						<ScrollArea className={optionSize === "small" ? styles.smallArea : undefined}>
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
