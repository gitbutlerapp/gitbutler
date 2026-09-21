import { defineMain } from "@storybook/react-vite/node";

export default defineMain({
	stories: [
		"../ui/src/**/*.stories.tsx",
		// The library's stories keep the "components/" titles, and so the story
		// ids, they had while they lived in ui/src/components.
		{
			directory: "../../../packages/ui-react/src",
			files: "**/*.stories.tsx",
			titlePrefix: "components",
		},
	],
	framework: "@storybook/react-vite",
	addons: ["@storybook/addon-designs", "@storybook/addon-docs"],
	typescript: {
		// Better props inference.
		reactDocgen: "react-docgen-typescript",
	},
});
