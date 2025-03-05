import type { EditorThemeClasses } from 'svelte-lexical';

import './component.css';
import './standard-theme.css';

export const standardTheme: EditorThemeClasses = {
	autocomplete: 'StandardTheme__autocomplete',
	blockCursor: 'StandardTheme__blockCursor',
	characterLimit: 'StandardTheme__characterLimit',
	code: 'StandardTheme__code',
	codeHighlight: {
		atrule: 'StandardTheme__tokenAttr',
		attr: 'StandardTheme__tokenAttr',
		boolean: 'StandardTheme__tokenProperty',
		builtin: 'StandardTheme__tokenSelector',
		cdata: 'StandardTheme__tokenComment',
		char: 'StandardTheme__tokenSelector',
		class: 'StandardTheme__tokenFunction',
		'class-name': 'StandardTheme__tokenFunction',
		comment: 'StandardTheme__tokenComment',
		constant: 'StandardTheme__tokenProperty',
		deleted: 'StandardTheme__tokenProperty',
		doctype: 'StandardTheme__tokenComment',
		entity: 'StandardTheme__tokenOperator',
		function: 'StandardTheme__tokenFunction',
		important: 'StandardTheme__tokenVariable',
		inserted: 'StandardTheme__tokenSelector',
		keyword: 'StandardTheme__tokenAttr',
		namespace: 'StandardTheme__tokenVariable',
		number: 'StandardTheme__tokenProperty',
		operator: 'StandardTheme__tokenOperator',
		prolog: 'StandardTheme__tokenComment',
		property: 'StandardTheme__tokenProperty',
		punctuation: 'StandardTheme__tokenPunctuation',
		regex: 'StandardTheme__tokenVariable',
		selector: 'StandardTheme__tokenSelector',
		string: 'StandardTheme__tokenSelector',
		symbol: 'StandardTheme__tokenProperty',
		tag: 'StandardTheme__tokenProperty',
		url: 'StandardTheme__tokenOperator',
		variable: 'StandardTheme__tokenVariable'
	},
	embedBlock: {
		base: 'StandardTheme__embedBlock',
		focus: 'StandardTheme__embedBlockFocus'
	},
	hashtag: 'StandardTheme__hashtag',
	heading: {
		h1: 'StandardTheme__h1',
		h2: 'StandardTheme__h2',
		h3: 'StandardTheme__h3',
		h4: 'StandardTheme__h4',
		h5: 'StandardTheme__h5',
		h6: 'StandardTheme__h6'
	},
	hr: 'StandardTheme__hr',
	image: 'editor-image',
	indent: 'StandardTheme__indent',
	inlineImage: 'inline-editor-image',
	layoutContainer: 'StandardTheme__layoutContainer',
	layoutItem: 'StandardTheme__layoutItem',
	link: 'StandardTheme__link',
	list: {
		checklist: 'StandardTheme__checklist',
		listitem: 'StandardTheme__listItem',
		listitemChecked: 'StandardTheme__listItemChecked',
		listitemUnchecked: 'StandardTheme__listItemUnchecked',
		nested: {
			listitem: 'StandardTheme__nestedListItem'
		},
		olDepth: [
			'StandardTheme__ol1',
			'StandardTheme__ol2',
			'StandardTheme__ol3',
			'StandardTheme__ol4',
			'StandardTheme__ol5'
		],
		ul: 'StandardTheme__ul'
	},
	ltr: 'StandardTheme__ltr',
	mark: 'StandardTheme__mark',
	markOverlap: 'StandardTheme__markOverlap',
	paragraph: 'StandardTheme__paragraph',
	quote: 'StandardTheme__quote',
	rtl: 'StandardTheme__rtl',
	text: {
		bold: 'StandardTheme__textBold',
		code: 'StandardTheme__textCode',
		italic: 'StandardTheme__textItalic',
		strikethrough: 'StandardTheme__textStrikethrough',
		subscript: 'StandardTheme__textSubscript',
		superscript: 'StandardTheme__textSuperscript',
		underline: 'StandardTheme__textUnderline',
		underlineStrikethrough: 'StandardTheme__textUnderlineStrikethrough'
	}
};
