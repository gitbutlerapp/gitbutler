import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { classes } from "#ui/components/classes.ts";
import { Tooltip } from "@base-ui/react";
import { Hash } from "effect";
import { FC } from "react";
import styles from "./ProjectButton.module.css";

type Props = {
	title: string;
	id: string;
	isSelected: boolean;
	onClick: () => void;
};

export const ProjectButton: FC<Props> = ({ title, id, isSelected, onClick }) => {
	const hue = ((Hash.string(id) % 360) + 360) % 360;
	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				render={
					<button
						type="button"
						aria-label={`Select project ${title}`}
						className={classes(styles.project, isSelected && styles.selected)}
						onClick={onClick}
						style={{ "--hue": hue }}
						disabled={isSelected}
					>
						<div className={styles.folderFront}>
							<span className={classes("text-bold", styles.folderFrontText)}>
								{" "}
								{title.slice(0, 2)}
							</span>
						</div>
					</button>
				}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4} side="right">
					<Tooltip.Popup render={<TooltipPopup content={title} />} />
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
