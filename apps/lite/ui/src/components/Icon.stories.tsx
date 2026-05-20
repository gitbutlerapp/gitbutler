import preview from "#storybook/preview";

import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames";

const iconNames = Object.keys(
	import.meta.glob("./icons/*.svg", {
		eager: true,
		query: "?raw",
		import: "default",
	}),
)
	.map((path) => path.replace(/^.*\/(.+)\.svg$/, "$1"))
	.sort((a, b) => a.localeCompare(b)) as Array<IconName>;

const meta = preview.meta({
	component: Icon,
	argTypes: {
		size: {
			control: { type: "range", min: 8, max: 128, step: 4 },
		},
	},
});

export const AllIcons = meta.story({
	args: {
		name: "commit",
		size: 16,
	} as never,
	render: ((args: { size: number }) => (
		<div
			style={{
				display: "grid",
				gridTemplateColumns: "repeat(auto-fill, minmax(100px, 1fr))",
				gap: 16,
			}}
		>
			{iconNames.map((name: IconName) => (
				<div
					key={name}
					style={{
						display: "flex",
						flexDirection: "column",
						// alignItems: "center",
						lineHeight: 1.3,
						gap: 12,
						padding: 16,
					}}
				>
					<Icon name={name} size={args.size} />
					<span style={{ fontSize: 11, opacity: 0.5 }}>{name}</span>
				</div>
			))}
		</div>
	)) as never,
});
