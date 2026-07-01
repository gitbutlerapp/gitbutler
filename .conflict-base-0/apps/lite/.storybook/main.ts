import { defineMain } from "@storybook/react-vite/node";

export default defineMain({
	stories: ["../ui/src/**/*.stories.tsx"],
	framework: "@storybook/react-vite",
	addons: ["@storybook/addon-designs", "@storybook/addon-docs"],
	typescript: {
		// Better props inference.
		reactDocgen: "react-docgen-typescript",
	},
});
