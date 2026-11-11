import { classes } from "./classes.ts";
import { personInitial } from "./personColour.ts";
import { usePicture } from "./usePicture.ts";
import type { CSSProperties, FC } from "react";
import styles from "./Avatar.module.css";

/** The sizes a person's picture is drawn at: 14 in a compact row, 16 inline, 18 heading a card. */
export type AvatarSize = 14 | 16 | 18;

/**
 * A person's picture in a circle. Without one, or when it fails to load, the real Gravatar
 * photo for an email `seed`; failing that, the person's initial on a colour picked from
 * `seed` (a login or an email), which also shows while a picture loads and is the colour
 * `ProfileImage` draws. Never a generated face, a blank or a broken image.
 * Decorative by default: the name beside it is what a screen reader reads, so give `alt`
 * only where it stands alone.
 * @figma The sample is the login "tj": purple, T.
 * @import import { Avatar } from "@gitbutler/but-ui-react/Avatar.tsx";
 */
export const Avatar: FC<{
	src: string | null | undefined;
	/** Who this is, for the fallback: an email finds their Gravatar, and either picks the colour and initial. */
	seed: string;
	size?: AvatarSize;
	alt?: string;
	className?: string;
}> = ({ src, seed, size = 16, alt = "", className }) => {
	const { url, tint, onError } = usePicture(src, seed, size);
	const style = { "--avatar-size": `${size}px`, backgroundColor: tint } as CSSProperties;
	return (
		<span
			className={classes(styles.avatar, className)}
			style={style}
			role={alt === "" ? undefined : "img"}
			aria-label={alt === "" ? undefined : alt}
		>
			<span aria-hidden className={styles.initial}>
				{personInitial(seed)}
			</span>
			{url !== null && <img src={url} alt="" className={styles.layer} onError={onError} />}
		</span>
	);
};
