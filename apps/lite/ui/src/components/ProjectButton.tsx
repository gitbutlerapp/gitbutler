import { Tooltip } from "#ui/components/Tooltip.tsx";
import { classes } from "#ui/components/classes.ts";
import { FC } from "react";
import styles from "./ProjectButton.module.css";

type Props = {
	title: string;
	hue: number;
	isSelected: boolean;
	onClick: () => void;
};

export const ProjectButton: FC<Props> = ({ title, hue, isSelected, onClick }) => (
	<Tooltip
		trigger={
			<button
				type="button"
				aria-label={`Select project ${title}`}
				className={classes(styles.project, isSelected && styles.selected)}
				onClick={onClick}
				style={{ "--hue": hue }}
				disabled={isSelected}
			>
				{title.slice(0, 2)}
			</button>
		}
		content={title}
		positionerProps={{ side: "right" }}
	/>
);
