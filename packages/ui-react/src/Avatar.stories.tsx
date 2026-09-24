import preview from "#storybook/preview";
import { Avatar, type AvatarSize } from "./Avatar.tsx";

const sizes: Array<AvatarSize> = [14, 16, 18];

const meta = preview.meta({
	component: Avatar,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2159-1542",
		},
	},
	args: { src: "https://avatars.githubusercontent.com/u/7453394?v=4", seed: "PavelLaptev" },
	argTypes: { size: { control: "select", options: sizes } },
});

export const Image = meta.story({});

/** No picture: the person's glitch, in a colour picked from the login, the same wherever they appear. */
export const NoPicture = meta.story({
	args: { src: null },
	render: (args) => (
		<div
			style={{
				display: "grid",
				gridTemplateColumns: "repeat(4, auto)",
				gap: 12,
				width: "fit-content",
			}}
		>
			{[
				"krlvi",
				"schacon",
				"Caleb-T-Owens",
				"mtsgrd",
				"slarse",
				"samhh",
				"estib-vega",
				"PavelLaptev",
			].map((login) => (
				<div key={login} style={{ display: "flex", gap: 8, alignItems: "center" }}>
					{sizes.map((size) => (
						<Avatar key={size} {...args} seed={login} size={size} />
					))}
				</div>
			))}
		</div>
	),
});

export const Sizes = meta.story({
	render: (args) => (
		<div style={{ display: "flex", gap: 12, alignItems: "center" }}>
			{sizes.map((size) => (
				<Avatar key={size} {...args} size={size} />
			))}
			{sizes.map((size) => (
				<Avatar key={`p${size}`} src={null} seed={args.seed} size={size} />
			))}
		</div>
	),
});

/** A picture that fails to load: the glitch takes its place, not a broken image. */
export const Broken = meta.story({ args: { src: "https://example.invalid/missing.png" } });
