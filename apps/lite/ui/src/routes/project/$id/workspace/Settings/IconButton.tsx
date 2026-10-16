import type { FC, MouseEvent, ReactNode } from "react";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { Tooltip } from "@gitbutler/ui-react/Tooltip.tsx";

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
	<Tooltip content={p.label}>
		<Button
			iconOnly={p.className === undefined}
			aria-label={p.label}
			disabled={p.disabled}
			className={p.className}
			onClick={p.onClick}
		>
			{p.children}
		</Button>
	</Tooltip>
);
