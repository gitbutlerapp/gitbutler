import autoprefixer from 'autoprefixer';
import postcssNesting from 'postcss-nesting';
import pxToRem from 'postcss-pxtorem';

export default {
	plugins: [
		autoprefixer(),
		pxToRem({
			rootValue: 16,
			unitPrecision: 5,
			propList: ['*'],
			selectorBlackList: [],
			replace: true,
			mediaQuery: false,
			minPixelValue: 0
		}),
		postcssNesting()
	]
};
