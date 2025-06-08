import autoprefixer from 'autoprefixer';
import postcssNesting from 'postcss-nesting';
import pxToRem from 'postcss-pxtorem';

export default {
	plugins: [
		autoprefixer(),
		pxToRem({
			rootValue: 16,
			mediaQuery: true
		}),
		postcssNesting()
	]
};
