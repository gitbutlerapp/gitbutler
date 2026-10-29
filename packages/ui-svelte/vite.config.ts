import { sveltekit } from "@sveltejs/kit/vite";
import path from "path";

export default {
	plugins: [sveltekit()],
	resolve: {
		alias: {
			$components: path.resolve("./src/lib/components"),
		},
	},
	test: {
		include: ["src/**/*.(test|spec).?(m)[jt]s?(x)"],
	},
	build: {
		sourcemap: true,
	},
};
