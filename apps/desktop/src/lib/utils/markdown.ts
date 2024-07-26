import { marked } from 'marked';

export function getMarkdownRenderer() {
	const renderer = new marked.Renderer({});
	renderer.link = function (href, title, text) {
		if (!title) title = text;
		return '<a target="_blank" href="' + href + '" title="' + title + '">' + text + '</a>';
	};
	return renderer;
}
