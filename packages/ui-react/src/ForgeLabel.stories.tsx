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

/** As dark as the dark theme's panel: the counterpart of the white label there. */
export const DarkColor = meta.story({
	args: { label: { name: "wontfix", color: "000000", description: null } },
});

/** This repository's own labels, to hold against the same row on github.com. */
export const RepositoryLabels = meta.story({
	args: { label: performanceLabel },
	render: (args) => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 6, maxWidth: 480 }}>
			{(
				[
					["dependencies", "0366d6"],
					["@gitbutler/desktop", "76AF01"],
					["@gitbutler/lite", "ededed"],
					["@gitbutler/ui-react", "f79614"],
					["@gitbutler/ui-svelte", "198E62"],
					["@gitbutler/web", "839FD1"],
					["javascript", "168700"],
					["@gitbutler/butler-bot", "5319e7"],
				] as const
			).map(([name, color]) => (
				<ForgeLabel key={name} size={args.size} label={{ name, color, description: null }} />
			))}
		</div>
	),
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
