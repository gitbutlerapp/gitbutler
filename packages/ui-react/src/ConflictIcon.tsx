import type { ComponentProps, FC } from "react";

type Props = {
	variant: "conflict" | "plus" | "minus";
	size?: number;
} & Omit<ComponentProps<"svg">, "width" | "height">;

export const ConflictIcon: FC<Props> = ({ variant, size = 16, ...props }) => {
	if (variant === "conflict") {
		return (
			<svg
				width={size}
				height={size}
				viewBox="0 0 16 16"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
				{...props}
			>
				<path
					d="M6.20252 2.82467C6.95455 1.40909 8.98295 1.40909 9.73498 2.82467L14.3765 11.5617C15.0842 12.8938 14.1187 14.5 12.6103 14.5H3.32721C1.8188 14.5 0.853298 12.8938 1.56098 11.5617L6.20252 2.82467Z"
					fill="var(--fill-danger-bg)"
				/>
				<path
					d="M7.96875 8.9375L7.96875 5.9375M7.96875 11.9375L7.96875 10.4375"
					stroke="white"
					strokeWidth="1.5"
				/>
			</svg>
		);
	}

	return (
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
				fill="var(--text-2)"
			/>
			<circle
				cx="13.7349"
				cy="5.5"
				r="5.5"
				fill={variant === "plus" ? "var(--fill-danger-bg)" : "var(--fill-safe-bg)"}
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
};
