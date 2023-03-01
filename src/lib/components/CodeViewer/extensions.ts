import { colorEditor, highLightSyntax } from './colors';
import { EditorView, lineNumbers } from '@codemirror/view';
const sizes = EditorView.theme({
    '&': { height: '100%', width: '100%' },
    '.cm-scroller': { overflow: 'scroll' }
});


export default [
    colorEditor,
    highLightSyntax,
    EditorView.editable.of(false),
    EditorView.lineWrapping,
    lineNumbers(),
    sizes
];
