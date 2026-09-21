import { Tooltip } from "@base-ui/react";
import type { FC, MouseEvent, ReactNode } from "react";
import { getButtonClassName } from "@gitbutler/ui-react/Button.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { TooltipPopup } from "@gitbutler/ui-react/Tooltip.tsx";

/** A button of glyphs alone, which says what it does on hover since they cannot. */
export const IconButton: FC<{
	label: string;
	/** For a button carrying two glyphs, which the one-glyph square cannot hold. */
	className?: string;
	disabled?: boolean;
	/** Handed the event, for a menu that opens where the button is. */
	onClick: (event: MouseEvent<HTMLButtonElement>) => void;
	children: ReactNode;
}> = (p) => (
	<Tooltip.Root>
		<Tooltip.Trigger
			className={classes(getButtonClassName({ iconOnly: p.className === undefined }), p.className)}
			render={<button type="button" aria-label={p.label} disabled={p.disabled} />}
			onClick={p.onClick}
		>
			{p.children}
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Positioner sideOffset={4}>
				<Tooltip.Popup render={<TooltipPopup />}>{p.label}</Tooltip.Popup>
			</Tooltip.Positioner>
		</Tooltip.Portal>
	</Tooltip.Root>
);
