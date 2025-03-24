export function smoothScroll(event: MouseEvent) {
	event.preventDefault();
	const link = event.currentTarget as HTMLAnchorElement;

	if (!link) {
		return;
	}

	const anchorId = new URL(link.href).hash.replace('#', '');
	const anchor = document.getElementById(anchorId);

	if (!anchor) {
		return;
	}

	window.scrollTo({
		top: anchor.offsetTop,
		behavior: 'smooth'
	});
}
