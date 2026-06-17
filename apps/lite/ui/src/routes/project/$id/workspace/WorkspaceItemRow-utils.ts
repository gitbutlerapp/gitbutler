import { ButtonVariant, getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import styles from "./WorkspaceItemRow.module.css";

export const getWorkspaceItemRowButtonClassName = ({
	variant = "ghost",
	iconOnly = false,
}: {
	variant?: Extract<ButtonVariant, "ghost" | "outline">;
	iconOnly?: boolean;
}) =>
	classes(
		getButtonClassName({
			variant,
			size: "small",
			iconOnly,
			// On selection/focus change we change the button variant. This
			// transition would clash with other selection/focus style changes
			// which are instant (e.g. the row background).
			disableTransition: true,
		}),
		(() => {
			switch (variant) {
				case "ghost":
					return styles.buttonGhost;
				case "outline":
					return styles.buttonOutline;
			}
		})(),
	);
