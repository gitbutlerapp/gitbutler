import type { ForgeReviewLabel } from "@gitbutler/but-sdk";
import type { FC } from "react";
import { Badge, type BadgeSize } from "./Badge.tsx";
import styles from "./ForgeLabel.module.css";

export const ForgeLabel: FC<{ label: ForgeReviewLabel; size?: BadgeSize }> = ({
	label,
	size = "large",
}) => {
	// GitHub sends bare hex color codes, GitLab prefixes them with #.
	const color =
		label.color === null ? null : label.color.startsWith("#") ? label.color : `#${label.color}`;
	return (
		<Badge
			variant="lightGray"
			size={size}
			className={styles.label}
			data-colored={color !== null || undefined}
			style={color === null ? undefined : { "--label-color": color }}
			title={
				label.description !== null && label.description !== ""
					? `${label.name}: ${label.description}`
					: label.name
			}
		>
			<span className={styles.text}>{label.name}</span>
		</Badge>
	);
};
