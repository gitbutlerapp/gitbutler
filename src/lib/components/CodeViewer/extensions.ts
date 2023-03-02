import { colorTheme, highlightStyle } from './colors';
import { EditorView, lineNumbers } from '@codemirror/view';
import { syntaxHighlighting } from '@codemirror/language';
import { gruvbox } from './themes';

const theme = gruvbox.dark;

const sizes = EditorView.theme({
	'&': { height: '100%', width: '100%' },
	'.cm-scroller': { overflow: 'scroll' }
});

export default [
	colorTheme(theme), // set color theme
	syntaxHighlighting(highlightStyle(theme)), // highlight syntax
	EditorView.editable.of(false), // disable editing
	EditorView.lineWrapping, // wrap lines
	lineNumbers(), // show line numbers
	sizes // set size
];
