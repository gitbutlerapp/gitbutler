import { marked } from 'marked';

export function getMarkdownRenderer() {
	const renderer = new marked.Renderer({});
	renderer.link = function (href, title, text) {
		if (!title) title = text;
		return `<Link this="Link" href="` + href + '" title="' + title + '">' + text + '</Link>';
	};
	return renderer;
}
