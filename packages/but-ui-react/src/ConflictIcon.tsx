import type { ComponentProps, FC } from "react";

type Props = {
	variant: "plus" | "minus";
	size?: number;
} & Omit<ComponentProps<"svg">, "width" | "height">;

/**
 * The paired marks on the two sides of a conflicted hunk: `plus` and `minus`. A conflicted commit
 * or file is marked with the `warning` icon in amber instead (but-ui's Conflicts note).
 *
 * @import import { ConflictIcon } from "@gitbutler/but-ui-react/ConflictIcon.tsx";
 */
export const ConflictIcon: FC<Props> = ({ variant, size = 16, ...props }) => (
	<svg
		width={(size * 20) / 16}
		height={size}
		viewBox="0 0 20 16"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		{...props}
	>
		<path
			opacity="0.8"
			d="M3.97745 4.07735C4.58318 2.86634 6.11122 2.65062 7.04581 3.42989C6.84356 4.08413 6.73526 4.77954 6.73526 5.50021C6.73537 8.58304 8.72915 11.1982 11.497 12.132C11.7228 13.3157 10.8237 14.5002 9.53018 14.5002H2.00186C0.515306 14.4999 -0.451071 12.9354 0.213776 11.6057L3.97745 4.07735Z"
			fill="var(--but-muted-foreground)"
		/>
		<circle
			cx="13.7349"
			cy="5.5"
			r="5.5"
			fill={variant === "plus" ? "var(--but-danger)" : "var(--but-success)"}
		/>
		{variant === "plus" ? (
			<path
				d="M13.7271 8.55545V2.44434M10.6792 5.49225L16.7903 5.49225"
				stroke="white"
				strokeWidth="1.5"
			/>
		) : (
			<path d="M10.6792 5.49219H16.7903" stroke="white" strokeWidth="1.5" />
		)}
	</svg>
);
