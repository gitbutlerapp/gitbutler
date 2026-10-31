import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import { PersonGlitch } from "./Avatar.tsx";
import { usePicture } from "./usePicture.ts";
import { useRef, type FC } from "react";
import styles from "./ProfileImage.module.css";

/**
 * The account's own picture, and the control that changes it: clicking it picks an image
 * file, and with a picture set, hovering offers a remove button in its corner. It holds no
 * state and uploads nothing; the host gets the chosen file and decides what to do with it.
 *
 * Without a picture it shows the account's Gravatar photo, else the account's glitch, the
 * one `Avatar` draws, which also shows while a picture loads. On the glitch alone the camera
 * always shows; over either, a dark wash comes on hover, the camera on it.
 * @import import { ProfileImage } from "@gitbutler/ui-react/ProfileImage.tsx";
 */
export const ProfileImage: FC<{
	/** The picture to show; without one, the Gravatar photo or the colour. */
	src: string | null | undefined;
	/** Who this is: an email finds their Gravatar, and either picks the colour. */
	seed: string;
	/** A file the person chose. */
	onChoose: (file: File) => void;
	/** Removes the picture. Without it, or without a picture, there is no remove button. */
	onRemove?: () => void;
	/** The image types the picker offers. */
	accept?: string;
	className?: string;
}> = ({ src, seed, onChoose, onRemove, accept = "image/png,image/jpeg", className }) => {
	const input = useRef<HTMLInputElement>(null);
	const hasPicture = src != null && src !== "";
	// A picture that fails to load still counts for the remove button; only what is drawn
	// falls back.
	const { url, tint, onError } = usePicture(src, seed, 72);

	return (
		<div className={classes(styles.root, className)}>
			<button
				type="button"
				className={styles.picture}
				// The person's colour, which also fills the circle while a picture loads.
				style={{ backgroundColor: tint }}
				aria-label="Change profile picture"
				onClick={() => input.current?.click()}
			>
				<PersonGlitch seed={seed} className={styles.glitch} />
				{url !== null ? (
					<>
						<img src={url} alt="" className={styles.image} onError={onError} />
						<span className={styles.overlay}>
							<Icon name="camera" className={styles.overlayIcon} size={32} />
						</span>
					</>
				) : (
					<>
						<span className={styles.overlay} />
						<Icon name="camera" className={styles.camera} size={32} />
					</>
				)}
			</button>
			{hasPicture && onRemove !== undefined && (
				<button
					type="button"
					className={styles.remove}
					aria-label="Remove profile picture"
					onClick={onRemove}
				>
					<Icon name="bin" />
				</button>
			)}
			<input
				ref={input}
				type="file"
				accept={accept}
				className={styles.fileInput}
				onChange={(evt) => {
					const file = evt.currentTarget.files?.[0];
					// Cleared so choosing the same file again still counts as a change, which
					// is what a retry after a failed save looks like.
					evt.currentTarget.value = "";
					if (file) onChoose(file);
				}}
			/>
		</div>
	);
};
