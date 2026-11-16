import { useAppearance } from "./gpui.ts";
import type { FC } from "react";

/**
 * The GPUI client's shot of a story, in the site's appearance. The shot is the example on this
 * ground with its 24px of room, so it fills the canvas in place of the story's stage.
 */
export const GpuiView: FC<{
	name: string;
	shots: { light: string; dark: string };
	hidden: boolean;
}> = ({ name, shots, hidden }) => (
	<img
		className="docs-gpui-shot"
		hidden={hidden}
		srcSet={`${shots[useAppearance()]} 2x`}
		alt={`${name}, as the GPUI client draws it`}
	/>
);
