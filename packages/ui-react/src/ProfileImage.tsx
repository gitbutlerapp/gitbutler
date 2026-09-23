import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import { useRef, type FC } from "react";
import styles from "./ProfileImage.module.css";

/**
 * The account's own picture, and the control that changes it: clicking it picks an image
 * file, and with a picture set, hovering offers a remove button in its corner. It holds no
 * state and uploads nothing; the host gets the chosen file and decides what to do with it.
 *
 * Without a picture it shows a person on the gray ground, which gives way to the camera
 * under the pointer; with one, a dark wash carries the camera over the picture.
 * @import import { ProfileImage } from "@gitbutler/ui-react/ProfileImage.tsx";
 */
export const ProfileImage: FC<{
	/** The picture to show; nothing for the placeholder. */
	src: string | null | undefined;
	/** A file the person chose. */
	onChoose: (file: File) => void;
	/** Removes the picture. Without it, or without a picture, there is no remove button. */
	onRemove?: () => void;
	/** The image types the picker offers. */
	accept?: string;
	className?: string;
}> = ({ src, onChoose, onRemove, accept = "image/png,image/jpeg", className }) => {
	const input = useRef<HTMLInputElement>(null);
	const hasPicture = src != null && src !== "";

	return (
		<div className={classes(styles.root, className)}>
			<button
				type="button"
				className={styles.picture}
				aria-label="Change profile picture"
				onClick={() => input.current?.click()}
			>
				{hasPicture ? (
					<>
						<img src={src} alt="" className={styles.image} />
						<span className={styles.overlay}>
							<Icon name="camera" className={styles.overlayIcon} size={32} />
						</span>
					</>
				) : (
					<>
						<Icon name="user" className={styles.placeholder} size={32} />
						<Icon name="camera" className={styles.placeholderCamera} size={32} />
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
