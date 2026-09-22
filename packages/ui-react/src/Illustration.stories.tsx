import preview from "#storybook/preview";
import { Illustration } from "./Illustration.tsx";
import { illustrations, type IllustrationName } from "./illustrations.ts";

const names = (Object.keys(illustrations) as Array<IllustrationName>).sort((a, b) =>
	a.localeCompare(b),
);

const meta = preview.type<{ args: { width: number } }>().meta({
	argTypes: {
		width: {
			control: { type: "range", min: 32, max: 256, step: 8 },
		},
	},
	args: { width: 96 },
});

export const AllIllustrations = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/EBuHQGUcCaSw4Ln5uVpWkn/Lite?node-id=4719-48274",
		},
	},
	render: (args) => (
		<div
			style={{
				display: "grid",
				gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))",
				gap: 16,
			}}
		>
			{names.map((name) => (
				<div
					key={name}
					style={{
						display: "flex",
						flexDirection: "column",
						alignItems: "center",
						gap: 12,
						padding: 16,
					}}
				>
					{/* On the app's paper rather than the story's ground: an illustration's
					    enclosed areas are --bg-1, so a swatch of anything else would hide
					    where the fill ends and the page begins. */}
					<div
						style={{
							display: "grid",
							placeItems: "center",
							width: "100%",
							padding: 24,
							borderRadius: 6,
							backgroundColor: "var(--bg-2)",
						}}
					>
						<Illustration name={name} width={args.width} />
					</div>
					<span style={{ fontSize: 11, lineHeight: 1.3, opacity: 0.5 }}>{name}</span>
				</div>
			))}
		</div>
	),
});
