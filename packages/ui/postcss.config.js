import postcssBundler from '@csstools/postcss-bundler';
import autoprefixer from 'autoprefixer';
import postcssMinify from 'postcss-minify';
import postcssNesting from 'postcss-nesting';
import pxToRem from 'postcss-pxtorem';

export default {
	plugins: [
		pxToRem({
			rootValue: 16,
			mediaQuery: true
		}),
		autoprefixer(),
		postcssNesting(),
		postcssMinify(),
		postcssBundler()
	]
};
