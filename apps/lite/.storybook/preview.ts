import docs from "@storybook/addon-docs";
import { definePreview } from "@storybook/react-vite";

export default definePreview({
	// Designs addon is missing as per:
	//   https://github.com/storybookjs/addon-designs/issues/277
	addons: [docs()],
	tags: ["autodocs"],
});
