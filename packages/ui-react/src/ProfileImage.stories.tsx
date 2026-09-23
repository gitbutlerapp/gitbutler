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
		onChoose: () => {},
	},
});

/** No picture yet: the person, which gives way to the camera under the pointer. */
export const Placeholder = meta.story({ args: { src: null } });

/** A picture, with the camera washed over it on hover. */
export const Picture = meta.story({});

/** A picture the host can remove: the corner button shows with the camera. */
export const Removable = meta.story({ args: { onRemove: () => {} } });
