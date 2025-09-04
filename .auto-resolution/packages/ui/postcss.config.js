import postcssBundler from '@csstools/postcss-bundler';
import autoprefixer from 'autoprefixer';
import cssnano from 'cssnano';
import postcssNesting from 'postcss-nesting';
import pxToRem from 'postcss-pxtorem';

/** @type {import('postcss').Config} */
const config = {
	plugins: [
		pxToRem({
			rootValue: 16,
			unitPrecision: 5,
			propList: ['*'],
			replace: true,
			mediaQuery: true
		}),
		autoprefixer(),
		postcssNesting(),
		postcssBundler(),
		...(process.env.NODE_ENV === 'production'
			? [
					cssnano({
						preset: ['default']
					})
				]
			: [])
	]
};

export default config;
