import preview from "#storybook/preview";
import { ProfileImage } from "./ProfileImage.tsx";

const meta = preview.meta({
	component: ProfileImage,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2156-6227",
		},
	},
	args: {
		src: "https://avatars.githubusercontent.com/u/7453394?v=4",
		seed: "PavelLaptev",
		onChoose: () => {},
	},
});

/** No picture and no Gravatar: the account's glitch, with the camera on it. */
export const NoPicture = meta.story({ args: { src: null } });

/** Each account gets its own colour and glitch, the same one every time. */
export const Accounts = meta.story({
	args: { src: null },
	render: (args) => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 16, maxWidth: 400 }}>
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
				<ProfileImage key={login} {...args} seed={login} />
			))}
		</div>
	),
});

/** A picture, with the camera washed over it on hover. */
export const Picture = meta.story({});

/** A picture the host can remove: the corner button shows with the camera. */
export const Removable = meta.story({ args: { onRemove: () => {} } });

/** A picture that fails to load: the glitch takes its place, not a broken image. */
export const Broken = meta.story({ args: { src: "https://example.invalid/missing.png" } });
