import { EditorView } from '@codemirror/view';
import { HighlightStyle } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';
import type { Theme } from './themes/theme';

export const colorTheme = (theme: Theme, options?: { dark: boolean }) =>
    EditorView.theme(
        {
            '&': {
                color: theme.fg0,
                backgroundColor: theme.bg0
            },
            '.cm-gutters': {
                color: theme.gray,
                backgroundColor: theme.bg0,
                border: 'none'
            },
            '.cm-mark': { backgroundColor: theme.bg2 }
        },
        options
    );

export const highlightStyle = (theme: Theme) =>
    HighlightStyle.define([
        { tag: t.tagName, color: theme.orange },
        { tag: t.keyword, color: theme.red },
        { tag: [t.propertyName, t.name, t.deleted, t.character, t.macroName], color: theme.blue },
        { tag: [t.function(t.variableName), t.labelName], color: theme.green, fontWeight: 'bold' },
        { tag: [t.definition(t.name), t.separator], color: theme.yellow },
        {
            tag: [
                t.typeName,
                t.className,
                t.number,
                t.changed,
                t.annotation,
                t.modifier,
                t.self,
                t.namespace
            ],
            color: theme.fg1
        },
        {
            tag: [t.operator, t.operatorKeyword, t.url, t.escape, t.regexp, t.link, t.special(t.string)],
            color: theme.orange,
            fontStyle: 'italic'
        },
        { tag: [t.meta, t.comment], color: theme.gray, fontStyle: 'italic' },
        { tag: t.strong, fontWeight: 'bold' },
        { tag: t.emphasis, fontStyle: 'italic' },
        { tag: t.strikethrough, textDecoration: 'line-through' },
        { tag: t.heading, fontWeight: 'bold', color: theme.blue },
        { tag: [t.atom, t.bool, t.special(t.variableName)], color: theme.purple },
        {
            tag: [t.processingInstruction, t.string, t.inserted],
            color: theme.green,
            fontStyle: 'italic'
        },
        { tag: t.invalid, color: theme.fg0 }
    ]);
