export function updateFavIcon(reviewStatus?: string) {
	// defaults
	let faviconUrlPng = '/favicon/favicon-64.png';
	let faviconUrlSvg = '/favicon/favicon.svg';

	// status icons
	if (reviewStatus) {
		if (reviewStatus === 'changes-requested') {
			faviconUrlPng = '/favicon/favicon-request-changes-64.png';
			faviconUrlSvg = '/favicon/favicon-request-changes.svg';
		} else if (reviewStatus === 'approved') {
			faviconUrlPng = '/favicon/favicon-approved-64.png';
			faviconUrlSvg = '/favicon/favicon-approved.svg';
		} else if (reviewStatus === 'in-discussion') {
			faviconUrlPng = '/favicon/favicon-in-discussion-64.png';
			faviconUrlSvg = '/favicon/favicon-in-discussion.svg';
		} else if (reviewStatus === 'unreviewed') {
			faviconUrlPng = '/favicon/favicon-unreviewed-64.png';
			faviconUrlSvg = '/favicon/favicon-unreviewed.svg';
		}
	}

	const linkPng = document.querySelector("link[id='favicon-png']") as HTMLLinkElement;
	if (linkPng) {
		linkPng.href = faviconUrlPng;
	}
	const linkSvg = document.querySelector("link[id='favicon-svg']") as HTMLLinkElement;
	if (linkSvg) {
		linkSvg.href = faviconUrlSvg;
	}
}
