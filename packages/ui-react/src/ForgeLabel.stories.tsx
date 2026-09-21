import preview from "#storybook/preview";
import { ForgeLabel } from "./ForgeLabel.tsx";

const performanceLabel = {
	name: "performance",
	color: "0e8a16",
	description: "Performance improvements",
};

const meta = preview.meta({
	component: ForgeLabel,
	args: { size: "regular" },
});

export const Regular = meta.story({ args: { label: performanceLabel } });

export const Large = meta.story({ args: { size: "large", label: performanceLabel } });

export const Colorless = meta.story({
	args: { label: { name: "needs review", color: null, description: null } },
});

export const LargeColorless = meta.story({
	args: { size: "large", label: { name: "needs review", color: null, description: null } },
});

export const PrefixedColor = meta.story({
	args: { label: { name: "@gitbutler/lite", color: "#ffffff", description: null } },
});

export const LongLabel = meta.story({
	args: {
		label: {
			name: "A long label that needs to fit in a narrow row",
			color: "dea584",
			description: null,
		},
	},
	render: (args) => (
		<div style={{ width: 160 }}>
			<ForgeLabel {...args} />
		</div>
	),
});
