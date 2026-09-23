import preview from "#storybook/preview";
import { Tag } from "./Tag.tsx";

const meta = preview.meta({
	component: Tag,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2143-1538",
		},
	},
	args: { name: "ui", color: "F2C230" },
});

export const Default = meta.story({});

export const Regular = meta.story({ args: { size: "regular" as const } });

/** No colour: the name alone, no dot. */
export const Colorless = meta.story({ args: { name: "needs review", color: null } });

/** A colour as light as the ground keeps its dot's shape; so does black in the dark theme. */
export const EdgeColours = meta.story({
	render: (args) => (
		<div style={{ display: "flex", gap: 6 }}>
			<Tag {...args} name="@gitbutler/lite" color="#ffffff" />
			<Tag {...args} name="wontfix" color="000000" />
		</div>
	),
});

/** This repository's labels and a Linear pair, to hold against the row they came from. */
export const Row = meta.story({
	render: (args) => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 6, maxWidth: 480 }}>
			{(
				[
					["ui", "F2C230"],
					["UX", "4FB3D1"],
					["dependencies", "0366d6"],
					["@gitbutler/desktop", "76AF01"],
					["@gitbutler/lite", "ededed"],
					["@gitbutler/ui-react", "f79614"],
					["javascript", "168700"],
					["@gitbutler/butler-bot", "5319e7"],
				] as const
			).map(([name, color]) => (
				<Tag key={name} {...args} name={name} color={color} />
			))}
		</div>
	),
});

export const LongName = meta.story({
	args: { name: "A long label that needs to fit in a narrow row", color: "dea584" },
	render: (args) => (
		<div style={{ width: 160 }}>
			<Tag {...args} />
		</div>
	),
});
