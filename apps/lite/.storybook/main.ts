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
		// The design notes, as a docs page, so the MCP server's docs tools serve them.
		"../../../packages/ui-react/DesignNotes.mdx",
	],
	framework: "@storybook/react-vite",
	features: {
		// /manifests/components.json (and .html to read): every component, its
		// props from the types, its stories and the import to write, for an agent
		// to read instead of guessing. Built from the TypeScript program this app's
		// tsconfig.json describes, which is why that file includes the library.
		componentsManifest: true,
		// Reads props through TypeScript's language service, and with it each
		// component's `@import` tag, so the manifest names the file to import
		// (`@gitbutler/ui-react/Avatar.tsx`) rather than a package root that has no
		// barrel. react-docgen-typescript drops the tag before the manifest sees it.
		experimentalReactComponentMeta: true,
	},
	addons: [
		"@storybook/addon-designs",
		"@storybook/addon-docs",
		// Runs axe on every story, in the Accessibility tab: contrast, missing names,
		// misused roles. A safety net for DESIGN.md's rules, not an audit.
		"@storybook/addon-a11y",
		// An MCP server at /mcp while Storybook runs, for an agent to list components
		// and read their props and stories from the manifest above. Chromatic
		// publishes its docs tools with each build.
		"@storybook/addon-mcp",
	],
	typescript: {
		// Better props inference.
		reactDocgen: "react-docgen-typescript",
	},
});
