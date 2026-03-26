import { FC } from "react";

export const CheckIcon: FC = () => (
	<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16">
		<path
			d="M3.5 8.5 6.5 11.5 12.5 5.5"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.5"
			strokeLinecap="round"
			strokeLinejoin="round"
		/>
	</svg>
);

export const MenuTriggerIcon: FC = () => (
	<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16">
		<circle cx="4" cy="8" r="1.25" fill="currentColor" />
		<circle cx="8" cy="8" r="1.25" fill="currentColor" />
		<circle cx="12" cy="8" r="1.25" fill="currentColor" />
	</svg>
);

export const DependencyIcon: FC = () => (
	<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16">
		<path
			d="M6.25 5.25H5a2.75 2.75 0 0 0 0 5.5h1.25"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.5"
			strokeLinecap="round"
			strokeLinejoin="round"
		/>
		<path
			d="M9.75 5.25H11a2.75 2.75 0 1 1 0 5.5H9.75"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.5"
			strokeLinecap="round"
			strokeLinejoin="round"
		/>
		<path
			d="M6.5 8h3"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.5"
			strokeLinecap="round"
			strokeLinejoin="round"
		/>
	</svg>
);

export const PushIcon: FC = () => (
	<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16">
		<path
			d="M8 9.5V5"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.5"
			strokeLinecap="round"
			strokeLinejoin="round"
		/>
		<path
			d="M6 5 8 3 10 5"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.5"
			strokeLinecap="round"
			strokeLinejoin="round"
		/>
		<path
			d="M4.5 11v.75C4.5 12.44 5.06 13 5.75 13h4.5c.69 0 1.25-.56 1.25-1.25V11"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.5"
			strokeLinecap="round"
			strokeLinejoin="round"
		/>
	</svg>
);

export const ExpandCollapseIcon: FC<{
	isExpanded: boolean;
}> = ({ isExpanded }) => (
	<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16">
		<path
			d="M4 8h8"
			stroke="currentColor"
			strokeWidth="1.5"
			strokeLinecap="round"
			strokeLinejoin="round"
		/>
		{!isExpanded && (
			<path
				d="M8 4v8"
				stroke="currentColor"
				strokeWidth="1.5"
				strokeLinecap="round"
				strokeLinejoin="round"
			/>
		)}
	</svg>
);
