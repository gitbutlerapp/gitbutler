import preview from "#storybook/preview";
import { Icon, icons } from "./Icon.tsx";

const iconNames = Array.from(icons.keys()).sort((a, b) => a.localeCompare(b));

const meta = preview.type<{ args: { size: number } }>().meta({
	argTypes: {
		size: {
			control: { type: "range", min: 8, max: 128, step: 4 },
		},
	},
});

export const AllIcons = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=231-330&t=ZB1Gk29sFC15sDSb-1",
		},
	},
	render: (args) => (
		<div
			style={{
				display: "grid",
				gridTemplateColumns: "repeat(auto-fill, minmax(100px, 1fr))",
				gap: 16,
			}}
		>
			{iconNames.map((name) => (
				<div
					key={name}
					style={{
						display: "flex",
						flexDirection: "column",
						gap: 12,
						padding: 16,
					}}
				>
					<Icon name={name} size={args.size} />
					<span style={{ fontSize: 11, lineHeight: 1.3, opacity: 0.5 }}>{name}</span>
				</div>
			))}
		</div>
	),
});
