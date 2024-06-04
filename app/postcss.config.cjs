const autoprefixer = require('autoprefixer');
const pxToRem = require('postcss-pxtorem');

const config = {
	plugins: [
		//But others, like autoprefixer, need to run after,
		autoprefixer,
		pxToRem({
			rootValue: 16,
			unitPrecision: 5,
			propList: ['*'],
			selectorBlackList: [],
			replace: true,
			mediaQuery: false,
			minPixelValue: 0
		})
	]
};

module.exports = config;
