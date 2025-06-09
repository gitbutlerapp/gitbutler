import autoprefixer from 'autoprefixer';
import cssnano from 'cssnano';
import postcssNesting from 'postcss-nesting';
import pxToRem from 'postcss-pxtorem';

export default {
	plugins: [
		autoprefixer(),
		pxToRem({
			rootValue: 16,
			mediaQuery: true
		}),
		postcssNesting(),
		...(process.env.NODE_ENV === 'production'
			? [
					cssnano({
						preset: ['default']
					})
				]
			: [])
	]
};
