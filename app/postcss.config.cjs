const tailwindcss = require('tailwindcss');
const autoprefixer = require('autoprefixer');
const nested = require('tailwindcss/nesting');

const config = {
	plugins: [
		//Makes it easier to define .dark theme classes
		nested,
		//Some plugins, like tailwindcss/nesting, need to run before Tailwind,
		tailwindcss(),
		//But others, like autoprefixer, need to run after,
		autoprefixer
	]
};

module.exports = config;
