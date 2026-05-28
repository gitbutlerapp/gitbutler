import { classes } from "#ui/components/classes.ts";
import styles from "./ToggleGroup.module.css";
import { ToggleGroup as BaseToggleGroup } from "@base-ui/react/toggle-group";
import { Toggle as BaseToggle } from "@base-ui/react/toggle";
import { ComponentProps } from "react";

type ToggleGroupProps = ComponentProps<typeof BaseToggleGroup>;

/** A segmented group of toggle buttons, styled with GitButler design tokens. */
export function ToggleGroup({ className, ...props }: ToggleGroupProps) {
	return (
		<BaseToggleGroup
			{...props}
			className={(state) =>
				classes(styles.group, typeof className === "function" ? className(state) : className)
			}
		/>
	);
}

type ToggleItemProps = ComponentProps<typeof BaseToggle>;

/** A single item within a {@link ToggleGroup}. */
export function ToggleItem({ className, ...props }: ToggleItemProps) {
	return (
		<BaseToggle
			{...props}
			className={(state) =>
				classes(
					"text-13",
					styles.item,
					typeof className === "function" ? className(state) : className,
				)
			}
		/>
	);
}
