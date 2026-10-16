import { classes } from "./classes.ts";
import { Match } from "effect";
import type { ComponentProps, FC } from "react";
import styles from "./FileStatusBadge.module.css";

/**
 * What happened to a file: the `type` of the SDK's `TreeStatus`, written out here so an app
 * without the SDK, like but.dev, can name it too.
 *
 * @public
 */
export type FileStatusType = "Addition" | "Deletion" | "Modification" | "Rename";

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
