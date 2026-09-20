import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { storybookSveltekitPlugin } from "@storybook/sveltekit/vite-plugin";

export default [
	"vite.config.ts",
	{
		extends: "vite.config.ts",
		plugins: [
			storybookTest({ storybookScript: "pnpm storybook --ci" }),
			storybookSveltekitPlugin(),
		],
		test: {
			browser: {
				enabled: true,
				headless: true,
				instances: [{ browser: "chromium" }],
				provider: "playwright",
			},
			setupFiles: ["./.storybook/vitest.setup.ts"],
		},
	},
];
