import { classes } from "./classes.ts";
import { GLITCH_SIZE, glitchPattern, personColour } from "./personColour.ts";
import { usePicture } from "./usePicture.ts";
import type { CSSProperties, FC } from "react";
import styles from "./Avatar.module.css";

/** The sizes a person's picture is drawn at: 14 in a compact row, 16 inline, 18 heading a card. */
export type AvatarSize = 14 | 16 | 18;

/**
 * The glitch a person without a picture is drawn with, in their colour's next step. It
 * fills its box and draws only the blocks: the caller's ground, the person's colour, shows
 * between them. `Avatar` and `ProfileImage` both use it, so a person is the same wherever
 * they appear, and it sits under a picture too, so a loading one shows the person's own
 * pattern until it lands.
 */
export const PersonGlitch: FC<{ seed: string; className?: string }> = ({ seed, className }) => (
	<svg
		aria-hidden
		className={className}
		viewBox={`0 0 ${GLITCH_SIZE} ${GLITCH_SIZE}`}
		preserveAspectRatio="none"
		shapeRendering="crispEdges"
	>
		<path d={glitchPattern(seed)} style={{ fill: personColour(seed).shade }} />
	</svg>
);

/**
 * A person's picture in a circle. Without one, or when it fails to load, the real Gravatar
 * photo for an email `seed`; failing that, the person's glitch in a colour picked from
 * `seed` (a login or an email), the same one that shows while a picture loads and the one
 * `ProfileImage` draws. Never a generated face, a blank or a broken image.
 * Decorative by default: the name beside it is what a screen reader reads, so give `alt`
 * only where it stands alone.
 * @import import { Avatar } from "@gitbutler/ui-react/Avatar.tsx";
 */
export const Avatar: FC<{
	src: string | null | undefined;
	/** Who this is, for the fallback: an email finds their Gravatar, and either picks the colour. */
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
			<PersonGlitch seed={seed} className={styles.layer} />
			{url !== null && <img src={url} alt="" className={styles.layer} onError={onError} />}
		</span>
	);
};
