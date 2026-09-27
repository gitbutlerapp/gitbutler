import { defineMain } from "@storybook/react-vite/node";

export default defineMain({
	stories: [
		// The design notes, one docs page per file under design/, so the MCP server's
		// docs tools serve them. Listed first, and group by group, for the sidebar's
		// reading order.
		"../../../packages/ui-react/design/overview.mdx",
		"../../../packages/ui-react/design/getting-started/*.mdx",
		"../../../packages/ui-react/design/foundations/*.mdx",
		"../../../packages/ui-react/design/content/*.mdx",
		"../../../packages/ui-react/design/components/*.mdx",
		"../../../packages/ui-react/design/patterns/*.mdx",
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
