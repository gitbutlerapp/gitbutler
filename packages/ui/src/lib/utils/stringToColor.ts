const colors = [
	'#E78D8D',
	'#62CDCD',
	'#EC90D2',
	'#7DC8D8',
	'#F1BC55',
	'#50D6AE',
	'#9785DE',
	'#99CE63',
	'#636ECE',
	'#5FD2B0'
];

export function stringToColor(name: string | undefined) {
	const trimmed = name?.replace(/\s/g, '');
	if (!trimmed) {
		return `linear-gradient(45deg, ${colors[0][0]} 15%, ${colors[0][1]} 90%)`;
	}

	const startHash = trimmed.split('').reduce((acc, char) => acc + char.charCodeAt(0), 0);

	return colors[startHash % colors.length];
}
