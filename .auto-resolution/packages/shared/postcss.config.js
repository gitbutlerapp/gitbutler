import autoprefixer from "autoprefixer";
import postcssNesting from "postcss-nesting";
import pxToRem from "postcss-pxtorem";

/** @type {import('postcss').Config} */
const config = {
	plugins: [
		pxToRem({
			rootValue: 16,
			unitPrecision: 5,
			propList: ["*"],
			replace: true,
			mediaQuery: true,
		}),
		autoprefixer(),
		postcssNesting(),
	],
};

export default config;
