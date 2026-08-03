import { useId, type ComponentProps, type FC } from "react";

/** Colored folder mark shown next to the project name in the workspace header. */
export const ProjectFolderIcon: FC<ComponentProps<"svg">> = (p) => {
	const gradientId = useId();

	return (
		<svg width="17" height="14" {...p} viewBox="0 0 17 14" fill="none" aria-hidden>
			<path
				d="M16.6629 13.2545H0V0H5.82617L7.57402 3.35636H16.6629V13.2545Z"
				fill={`url(#${gradientId})`}
			/>
			<defs>
				<linearGradient
					id={gradientId}
					x1="7.57403"
					y1="0"
					x2="7.57403"
					y2="11.7397"
					gradientUnits="userSpaceOnUse"
				>
					<stop stopColor="#BCE1E2" />
					<stop offset="1" stopColor="#7BC6C7" />
				</linearGradient>
			</defs>
		</svg>
	);
};
