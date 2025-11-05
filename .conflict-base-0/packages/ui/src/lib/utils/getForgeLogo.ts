import type iconsJson from '$lib/data/icons.json';

export function getForgeLogo(forgeName: string, small = false): keyof typeof iconsJson {
	if (forgeName === 'gitlab') {
		if (small) {
			return 'gitlab-small';
		}
		return 'gitlab';
	} else if (forgeName === 'github') {
		if (small) {
			return 'github-outline-small';
		}

		return 'github-outline';
	}

	return 'question-mark-small';
}
