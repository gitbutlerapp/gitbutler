// Copyright 2021 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import { HighlightStyle, type LanguageSupport } from '@codemirror/language';
import { tags, highlightTree } from '@lezer/highlight';
import { NodeType, Tree } from '@lezer/common';
import { javascript } from '@codemirror/lang-javascript';
import { css } from '@codemirror/lang-css';
import { html } from '@codemirror/lang-html';
import { xml } from '@codemirror/lang-xml';
import { cpp } from '@codemirror/lang-cpp';
import { java } from '@codemirror/lang-java';
import { json } from '@codemirror/lang-json';
import { php } from '@codemirror/lang-php';
import { python } from '@codemirror/lang-python';
import { markdown } from '@codemirror/lang-markdown';
import { wast } from '@codemirror/lang-wast';
// import { svelte } from '@replit/codemirror-lang-svelte';
import { vue } from '@codemirror/lang-vue';
import { rust } from '@codemirror/lang-rust';

const t = tags;

export const highlightStyle: HighlightStyle = HighlightStyle.define([
	{ tag: t.variableName, class: 'token-variable' },
	{ tag: t.definition(t.variableName), class: 'token-definition' },
	{ tag: t.propertyName, class: 'token-property' },
	{ tag: [t.typeName, t.className, t.namespace, t.macroName], class: 'token-type' },
	{ tag: [t.special(t.name), t.constant(t.className)], class: 'token-variable-special' },
	{ tag: t.standard(t.variableName), class: 'token-builtin' },

	{ tag: [t.number, t.literal, t.unit], class: 'token-number' },
	{ tag: t.string, class: 'token-string' },
	{ tag: [t.special(t.string), t.regexp, t.escape], class: 'token-string-special' },
	{ tag: [], class: 'token-atom' },

	{ tag: t.keyword, class: 'token-keyword' },
	{ tag: [t.comment, t.quote], class: 'token-comment' },
	{ tag: t.meta, class: 'token-meta' },
	{ tag: t.invalid, class: 'token-invalid' },

	{ tag: t.tagName, class: 'token-tag' },
	{ tag: t.attributeName, class: 'token-attribute' },
	{ tag: t.attributeValue, class: 'token-attribute-value' },

	{ tag: t.inserted, class: 'token-inserted' },
	{ tag: t.deleted, class: 'token-deleted' },
	{ tag: t.heading, class: 'token-heading' },
	{ tag: t.link, class: 'token-link' },
	{ tag: t.strikethrough, class: 'token-strikethrough' },
	{ tag: t.strong, class: 'token-strong' },
	{ tag: t.emphasis, class: 'token-emphasis' }
]);

export function create(code: string, filepath: string): CodeHighlighter {
	const language = languageFromFilename(filepath);
	let tree: Tree;
	if (language) {
		tree = language.language.parser.parse(code);
	} else {
		tree = new Tree(NodeType.none, [], [], code.length);
	}
	return new CodeHighlighter(code, tree);
}

export function highlightNode(node: Element, mimeType: string): void {
	const code = node.textContent || '';
	const highlighter = create(code, mimeType);
	if (node.firstChild) {
		node.textContent = '';
	}
	highlighter.highlight((text, style) => {
		let token: Node = document.createTextNode(text);
		if (style) {
			const span = document.createElement('span');
			span.className = style;
			span.appendChild(token);
			token = span;
		}
		node.appendChild(token);
	});
}

export function languageFromFilename(filename: string): LanguageSupport | null {
	const ext = filename.split('.').pop();
	switch (ext) {
		case 'jsx':
		case 'js':
			// We intentionally allow JSX in normal .js as well as .jsx files,
			// because there are simply too many existing applications and
			// examples out there that use JSX within .js files, and we don't
			// want to break them.
			return javascript({ jsx: true });
		case 'ts':
			return javascript({ typescript: true });
		case 'tsx':
			return javascript({ typescript: true, jsx: true });

		case 'css':
			return css();

		case 'html':
			return html({ selfClosingTags: true });

		case 'xml':
			return xml();

		case 'wasm':
			return wast();

		case 'cpp':
		case 'c++':
		case 'hpp':
		case 'h++':
			return cpp();

		// case 'text/x-go':
		//     return new LanguageSupport(await CodeMirror.go());

		case 'java':
			return java();

		// case 'text/x-kotlin':
		//     return new LanguageSupport(await CodeMirror.kotlin());

		case 'json':
			return json();

		case 'php':
			return php();

		case 'python':
			return python();

		case 'md':
			return markdown();

		// case 'text/x-sh':
		//     return new LanguageSupport(await CodeMirror.shell());

		// case 'text/x-coffeescript':
		//     return new LanguageSupport(await CodeMirror.coffeescript());

		// case 'text/x-clojure':
		//     return new LanguageSupport(await CodeMirror.clojure());

		// case 'application/vnd.dart':
		//     return new LanguageSupport(await CodeMirror.dart());

		// case 'text/x-gss':
		//     return new LanguageSupport(await CodeMirror.gss());

		// case 'text/x-less':
		//     return new CodeMirror.LanguageSupport(await CodeMirror.less());

		// case 'text/x-sass':
		//     return new LanguageSupport(await CodeMirror.sass());

		// case 'text/x-scala':
		//     return new LanguageSupport(await CodeMirror.scala());

		// case 'text/x-scss':
		//     return new LanguageSupport(await CodeMirror.scss());

		case 'svelte':
			// TODO: is codemirror-lang-svelte broken or just not used correctly?
			// return svelte();

			// highlighting svelte with js + jsx works much better than the above
			return javascript({ typescript: true, jsx: true });

		case 'vue':
			return vue();

		case 'rs':
			return rust();

		default:
			return null;
	}
}

export class CodeHighlighter {
	constructor(
		readonly code: string,
		readonly tree: Tree
	) {}

	highlight(token: (text: string, style: string) => void): void {
		this.highlightRange(0, this.code.length, token);
	}

	highlightRange(from: number, to: number, token: (text: string, style: string) => void): void {
		let pos = from;
		const flush = (to: number, style: string): void => {
			if (to > pos) {
				token(this.code.slice(pos, to), style);
				pos = to;
			}
		};
		highlightTree(
			this.tree,
			highlightStyle,
			(from, to, style) => {
				flush(from, '');
				flush(to, style);
			},
			from,
			to
		);
		flush(to, '');
	}
}
