import styles from "./ProgramIcon.module.css";
import { classes } from "#ui/components/classes.ts";
import { programIconFor } from "./programIcons.ts";
import type { ComponentProps, FC } from "react";

type Props = {
	/** The identifier the backend lists the program under: `vscode`, `warp`, `iterm2`. */
	program: string;
	/** Width and height in px. */
	size?: number;
} & Omit<ComponentProps<"img">, "src" | "alt">;

/**
 * The mark of an editor or terminal, for a control that names one. Renders nothing for a
 * program without a mark — a user-configured editor, a terminal the library has not drawn —
 * so a caller can place it unconditionally and let the text carry the name alone.
 *
 * @public
 */
export const ProgramIcon: FC<Props> = ({ program, size, ...props }) => {
	const src = programIconFor(program);
	if (src === undefined) return null;
	return (
		<img
			{...props}
			src={src}
			alt=""
			className={classes(props.className, styles.programIcon)}
			style={{
				...props.style,
				...(size !== undefined ? { "--program-icon-size": `${size}px` } : undefined),
			}}
		/>
	);
};
