import { useState, type FC } from "react";
import { stories } from "../docs/stories.ts";
import { Icon } from "./Icon.tsx";
import { icons } from "./icons.ts";
import { Range } from "./Range.tsx";

const iconNames = Array.from(icons.keys()).sort((a, b) => a.localeCompare(b));

const meta = stories<{ size: number }>({
	title: "Icons/Icon",
	id: "components-icon",
});

/** One icon, at 16px. */
export const Default = meta.story({
	args: { size: 16 },
	render: (args) => <Icon name="branch" size={args.size} />,
});

export const AllIcons = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=231-330&t=ZB1Gk29sFC15sDSb-1",
		},
		layout: "wide",
	},
	render: () => <Gallery />,
});

/** Every icon by name, at a size picked with the range above them: 16px is the grid they're drawn
 * on, and a larger size shows how a shape holds up. */
const Gallery: FC = () => {
	const [size, setSize] = useState(16);
	return (
		<div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
			<Range
				label="Size"
				value={size}
				onValueChange={setSize}
				min={12}
				max={48}
				step={2}
				marks={[16, 24, 32, 40, 48]}
				style={{ width: 240 }}
			/>
			<div
				style={{
					display: "grid",
					gridTemplateColumns: `repeat(auto-fill, minmax(${Math.max(100, size + 32)}px, 1fr))`,
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
						<Icon name={name} size={size} />
						<span style={{ fontSize: 11, lineHeight: 1.3, opacity: 0.5 }}>{name}</span>
					</div>
				))}
			</div>
		</div>
	);
};
