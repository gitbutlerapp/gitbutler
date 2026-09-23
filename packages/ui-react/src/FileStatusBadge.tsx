import { classes } from "./classes.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";
import type { ComponentProps, FC } from "react";
import styles from "./FileStatusBadge.module.css";

/** @public */
export type FileStatusType = TreeChange["status"]["type"];

type Props = {
	status: FileStatusType;
} & ComponentProps<"span">;

/**
 * @import import { FileStatusBadge } from "@gitbutler/ui-react/FileStatusBadge.tsx";
 */
export const FileStatusBadge: FC<Props> = ({ status, ...props }) => (
	<span
		aria-label={status}
		{...props}
		className={classes(props.className, styles.badge)}
		data-status-type={status}
	>
		{Match.value(status).pipe(
			Match.when("Addition", () => "A"),
			Match.when("Deletion", () => "D"),
			Match.when("Modification", () => "M"),
			Match.when("Rename", () => "R"),
			Match.exhaustive,
		)}
	</span>
);
