import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import styles from "./WorkspaceItemRow.module.css";

export const getWorkspaceItemRowButtonClassName = ({ iconOnly = false }: { iconOnly?: boolean }) =>
	classes(
		getButtonClassName({
			variant: "ghost",
			size: "small",
			iconOnly,
			// On selection/focus change we change the button variant. This
			// transition would clash with other selection/focus style changes
			// which are instant (e.g. the row background).
			disableTransition: true,
		}),
		styles.button,
	);
