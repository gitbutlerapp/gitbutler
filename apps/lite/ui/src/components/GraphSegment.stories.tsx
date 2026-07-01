import preview from "#storybook/preview";
import { GraphSegmentGlyph, GraphSegment } from "./GraphSegment.tsx";

const meta = preview.meta({
	component: GraphSegment,
	argTypes: {
		glyph: {
			control: { type: "select" },
			options: [
				"parent",
				"horizontal",
				"space",
				"forkLeft",
				"forkRight",
				"forkBoth",
				"mergeLeft",
				"mergeRight",
				"mergeBoth",
				"joinLeft",
				"joinRight",
				"joinBoth",
				"commit",
				"group",
			],
		},
	},
});

export const Default = meta.story({
	args: {
		glyph: "parent",
		status: "LocalOnly",
	},
	render: (args) => (
		<div
			style={{
				height: 100,
				backgroundColor: "var(--bg-2)",
				display: "flex",
			}}
		>
			<GraphSegment {...args} />
		</div>
	),
});

export const AllGlyphs = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 16 }}>
			{(
				[
					"parent",
					"horizontal",
					"forkLeft",
					"forkRight",
					"forkBoth",
					"mergeLeft",
					"mergeRight",
					"mergeBoth",
					"joinLeft",
					"joinRight",
					"joinBoth",
					"commit",
					"group",
					"space",
				] satisfies Array<GraphSegmentGlyph>
			).map((glyph) => (
				<div
					key={glyph}
					style={{
						display: "flex",
						alignItems: "flex-start",
						flexDirection: "column",
						gap: 12,
					}}
				>
					<div style={{ backgroundColor: "var(--bg-2)", display: "flex" }}>
						<GraphSegment glyph={glyph} status="LocalOnly" />
					</div>
					<div style={{ fontSize: 10 }}>{glyph}</div>
				</div>
			))}
		</div>
	),
});
