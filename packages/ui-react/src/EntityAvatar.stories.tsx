import preview from "#storybook/preview";
import { useState } from "react";
import { EntityAvatar, EntityAvatarPicker } from "./EntityAvatar.tsx";
import {
	entityAvatarColours,
	entityAvatarEmojiSection,
	entityAvatarGlyphSection,
	type EntityAvatarSection,
	type EntityAvatarValue,
} from "./entityAvatarChoices.ts";
import { machinePictures } from "./story-assets/machines.ts";

const meta = preview.meta({
	title: "Content & status/EntityAvatar",
	id: "components-entityavatar",
	component: EntityAvatar,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=2793-14027",
		},
	},
	argTypes: {
		size: { control: "radio", options: ["regular", "title"] },
	},
});

export const Default = meta.story({
	args: {
		value: { _tag: "Glyph", colour: "outline" },
		size: "regular",
	},
});

/** The glyph in each of its colours, an emoji and a machine's picture: every avatar is 38px. */
export const Choices = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=2793-14169",
		},
	},
	render: () => (
		<div style={{ display: "flex", gap: 8, alignItems: "center" }}>
			{entityAvatarColours.map((colour) => (
				<EntityAvatar key={colour} value={{ _tag: "Glyph", colour }} />
			))}
			<EntityAvatar value={{ _tag: "Emoji", emoji: "☁️" }} />
			<EntityAvatar value={{ _tag: "Picture", src: machinePictures[0].src }} />
		</div>
	),
});

const machineSection: EntityAvatarSection = {
	name: "Machines",
	size: "regular",
	options: machinePictures.map(({ name, src }) => ({ value: { _tag: "Picture", src }, name })),
};

const ProjectPicker = () => {
	const [value, setValue] = useState<EntityAvatarValue>({ _tag: "Glyph", colour: "blue" });
	return (
		<EntityAvatarPicker
			value={value}
			onChange={setValue}
			label="Change gitbutler's avatar"
			sections={[entityAvatarGlyphSection, entityAvatarEmojiSection]}
			searchPlaceholder="Search emoji…"
		/>
	);
};

/** A project's avatar: a colour for the glyph, or an emoji found by name. */
export const PickingForAProject = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=2793-14206",
		},
	},
	render: () => <ProjectPicker />,
});

const MachinePicker = () => {
	const [value, setValue] = useState<EntityAvatarValue>({
		_tag: "Picture",
		src: machinePictures[0].src,
	});
	return (
		<EntityAvatarPicker
			value={value}
			onChange={setValue}
			label="Change pave--macbook-pro's picture"
			sections={[machineSection]}
		/>
	);
};

/** A machine's picture: few enough kinds to show them at full size in the picker, with no search. */
export const PickingForAMachine = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=2793-14336",
		},
	},
	render: () => <MachinePicker />,
});
