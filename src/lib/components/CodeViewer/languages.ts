import type { Extension } from '@codemirror/state';

import { javascript } from '@codemirror/lang-javascript';
import { rust } from '@codemirror/lang-rust';
import { markdown } from '@codemirror/lang-markdown';
import { python } from '@codemirror/lang-python';
import { html } from '@codemirror/lang-html';
import { json } from '@codemirror/lang-json';
import { php } from '@codemirror/lang-php';
import { java } from '@codemirror/lang-java';
import { css } from '@codemirror/lang-css';
import { svelte } from '@replit/codemirror-lang-svelte';
import { vue } from '@codemirror/lang-vue';
import { angular } from '@codemirror/lang-angular';
import { csharp } from '@replit/codemirror-lang-csharp';
import { clojure } from '@nextjournal/lang-clojure';

const supported = new Map<string, Extension>([
	['.js', javascript({ jsx: false, typescript: false })],
	['.ts', javascript({ jsx: false, typescript: true })],
	['.jsx', javascript({ jsx: true, typescript: false })],
	['.tsx', javascript({ jsx: true, typescript: true })],
	['.rs', rust()],
	['.md', markdown()],
	['.py', python()],
	['.html', html()],
	['.json', json()],
	['.php', php()],
	['.java', java()],
	['.css', css()],
	['.svelte', svelte()],
	['.vue', vue()],
	['.angular', angular()],
	['.cs', csharp()],
	['.clj', clojure()]
]);

export const getLanguage = (filepath: string) =>
	supported.get(filepath.slice(filepath.lastIndexOf('.')));
