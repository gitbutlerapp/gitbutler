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
	features: {
		// /manifests/components.json (and .html to read): every component, its
		// props from the types, its stories and the import to write, for an agent
		// to read instead of guessing. Built from the TypeScript program this app's
		// tsconfig.json describes, which is why that file includes the library.
		componentsManifest: true,
	},
	addons: ["@storybook/addon-designs", "@storybook/addon-docs"],
	typescript: {
		// Better props inference.
		reactDocgen: "react-docgen-typescript",
	},
});
