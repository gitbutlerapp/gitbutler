import type { PromptTemplate } from '$lib/codegen/types';

export type ParsedTemplateMetadata = {
	name: string | null;
	emoji: string | null;
	content: string;
};

export type ParsedTemplate = {
	fileName: string;
	template: string;
	parsed: ParsedTemplateMetadata;
};

export type TemplateForDisplay = {
	label: string;
	emoji: string | undefined;
	fileName: string;
};

/**
 * Parse YAML frontmatter from template.
 * Format:
 * ---
 * name: My Template
 * emoji: 🚀
 * ---
 *
 * Template content here...
 */
export function parseTemplateMetadata(template: string): ParsedTemplateMetadata {
	const frontmatterRegex = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/;
	const match = template.match(frontmatterRegex);

	if (!match) {
		// No frontmatter found, return template as-is
		return { name: null, emoji: null, content: template };
	}

	const [, frontmatter = '', content = ''] = match;
	const name = frontmatter.match(/^name:\s*(.+)$/m)?.[1]?.trim() || null;
	const emoji = frontmatter.match(/^emoji:\s*(.+)$/m)?.[1]?.trim() || null;

	return { name, emoji, content: content.trim() };
}

/**
 * Parse an array of PromptTemplate objects into ParsedTemplate objects
 */
export function parseTemplates(templates: PromptTemplate[]): ParsedTemplate[] {
	return templates.map((t) => {
		const parsed = parseTemplateMetadata(t.template);
		return {
			fileName: t.fileName,
			template: t.template,
			parsed: {
				name: parsed.name,
				emoji: parsed.emoji,
				content: parsed.content
			}
		};
	});
}

/**
 * Convert parsed templates to display format with label and emoji
 */
export function templatesToDisplayFormat(templates: ParsedTemplate[]): TemplateForDisplay[] {
	return templates.map((t) => ({
		label: t.parsed.name || t.fileName,
		emoji: t.parsed.emoji || undefined,
		fileName: t.fileName
	}));
}
