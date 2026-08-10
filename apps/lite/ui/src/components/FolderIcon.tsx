import styles from "./FolderIcon.module.css";
import { useId, type ComponentProps, type FC } from "react";

/**
 * The folder mark, wherever a folder is named: the workspace header and every
 * directory row in the file tree. Callers size it with CSS; it keeps its
 * proportions inside whatever box they give it.
 */
export const FolderIcon: FC<ComponentProps<"svg">> = (p) => {
	// A directory row renders one of these, so many copies share the DOM and the
	// gradient needs an id that cannot collide between them.
	const gradientId = useId();

	return (
		<svg width="17" height="14" {...p} viewBox="0 0 17 14" fill="none" aria-hidden>
			<path
				d="M16.6629 13.2545H0V0H5.82617L7.57402 3.35636H16.6629V13.2545Z"
				fill={`url(#${gradientId})`}
			/>
			<defs>
				<linearGradient
					id={gradientId}
					x1="7.57403"
					y1="0"
					x2="7.57403"
					y2="11.7397"
					gradientUnits="userSpaceOnUse"
				>
					<stop className={styles.top} />
					<stop offset="1" className={styles.bottom} />
				</linearGradient>
			</defs>
		</svg>
	);
};
