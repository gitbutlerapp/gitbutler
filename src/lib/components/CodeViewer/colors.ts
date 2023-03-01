import { EditorView } from '@codemirror/view';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { tags } from '@lezer/highlight';

const palette = {
	invalid: '#ffffff',
	coral: '#e06c75',
	chalky: '#e5c07b',
	cyan: '#56b6c2',
	ivory: '#abb2bf',
	stone: '#5c6370',
	malibu: '#61afef',
	sage: '#98c379',
	whiskey: '#d19a66',
	violet: '#c678dd'
};

export const colorEditor = EditorView.theme(
	{
		'&': {
			color: '#d4d4d8',
			backgroundColor: '#18181b'
		},
		'.cm-gutters': {
			backgroundColor: '#18181b',
			color: '#3f3f46'
		}
	},
	{ dark: true }
);

export const highLightSyntax = syntaxHighlighting(
	HighlightStyle.define([
		{
			tag: tags.keyword,
			color: palette.violet
		},
		{
			tag: [tags.name, tags.deleted, tags.character, tags.propertyName, tags.macroName],
			color: palette.coral
		},
		{
			tag: [tags.processingInstruction, tags.string, tags.inserted],
			color: palette.sage
		},
		{
			tag: [tags.function(tags.variableName), tags.labelName],
			color: palette.malibu
		},
		{
			tag: [tags.color, tags.constant(tags.name), tags.standard(tags.name)],
			color: palette.whiskey
		},
		{
			tag: [tags.definition(tags.name), tags.separator],
			color: palette.ivory
		},
		{
			tag: [
				tags.typeName,
				tags.className,
				tags.number,
				tags.changed,
				tags.annotation,
				tags.modifier,
				tags.self,
				tags.namespace
			],
			color: palette.chalky
		},
		{
			tag: [
				tags.operator,
				tags.operatorKeyword,
				tags.url,
				tags.escape,
				tags.regexp,
				tags.link,
				tags.special(tags.string)
			],
			color: palette.cyan
		},
		{
			tag: [tags.meta, tags.comment],
			color: palette.stone
		},
		{
			tag: tags.strong,
			fontWeight: 'bold'
		},
		{
			tag: tags.emphasis,
			fontStyle: 'italic'
		},
		{
			tag: tags.link,
			color: palette.stone,
			textDecoration: 'underline'
		},
		{
			tag: tags.heading,
			fontWeight: 'bold',
			color: palette.coral
		},
		{
			tag: [tags.atom, tags.bool, tags.special(tags.variableName)],
			color: palette.whiskey
		},
		{
			tag: tags.invalid,
			color: palette.invalid
		}
	])
);
