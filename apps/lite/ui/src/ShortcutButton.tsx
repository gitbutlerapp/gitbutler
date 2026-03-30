import { type ComponentProps } from "react";
import {
	formatShortcutKeys,
	type ShortcutActionBase,
	type ShortcutBinding,
} from "#ui/shortcuts.ts";

export const ShortcutButton = ({
	binding,
	...props
}: Omit<ComponentProps<"button">, "children" | "type"> & {
	binding: ShortcutBinding<ShortcutActionBase>;
}) => (
	<button {...props} type="button">
		{binding.description} ({formatShortcutKeys(binding.keys)})
	</button>
);
