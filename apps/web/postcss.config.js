import postcssGlobalData from '@csstools/postcss-global-data';
import autoprefixer from 'autoprefixer';
import postcssCustomMedia from 'postcss-custom-media';
import postcssNesting from 'postcss-nesting';
import pxToRem from 'postcss-pxtorem';
import path from 'path';

const mediaQueriesCssPath = path.resolve('src/lib/styles/media-queries.css');

/** @type {import('postcss-load-config').Config} */
const config = {
	plugins: [
		autoprefixer(),
		pxToRem({
			rootValue: 16,
			unitPrecision: 5,
			propList: ['*'],
			selectorBlackList: [],
			replace: true,
			mediaQuery: true,
			minPixelValue: 0
		}),
		postcssNesting(),
		postcssGlobalData({
			files: [mediaQueriesCssPath]
		}),
		postcssCustomMedia()
	]
};

export default config;
