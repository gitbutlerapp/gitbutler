import { classes } from "#ui/components/classes.ts";
// A folder sits in the same slot as a file's type icon and is drawn to the same
// box, so it borrows that component's sizing rather than restating it.
import styles from "./FileIcon.module.css";
import type { ComponentProps, FC } from "react";

export const FolderIcon: FC<ComponentProps<"i">> = (props) => (
	<i {...props} className={classes(props.className, styles.fileIcon)} aria-hidden>
		<svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path d="M14 13H0V1H4.8951L6.36364 4.05641H14V13Z" fill="url(#folder-icon-fill)" />
			<defs>
				<linearGradient
					id="folder-icon-fill"
					x1="6.36364"
					y1="1"
					x2="6.36364"
					y2="11.6286"
					gradientUnits="userSpaceOnUse"
				>
					<stop stopColor="#BCE1E2" />
					<stop offset="1" stopColor="#7BC6C7" />
				</linearGradient>
			</defs>
		</svg>
	</i>
);
