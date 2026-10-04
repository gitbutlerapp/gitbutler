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
	component: EntityAvatar,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/EBuHQGUcCaSw4Ln5uVpWkn/Client?node-id=5612-128983",
		},
	},
	argTypes: {
		size: { control: "radio", options: ["regular", "small", "large"] },
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
	size: "large",
	options: machinePictures.map(({ name, src }) => ({ value: { _tag: "Picture", src }, name })),
};

const ProjectPicker = () => {
	const [value, setValue] = useState<EntityAvatarValue>({ _tag: "Glyph", colour: "pop" });
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

/** A machine's picture: few enough kinds to show them large in the picker, with no search. */
export const PickingForAMachine = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/EBuHQGUcCaSw4Ln5uVpWkn/Client?node-id=5685-58547",
		},
	},
	render: () => <MachinePicker />,
});
