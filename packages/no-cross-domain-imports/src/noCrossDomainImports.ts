/**
 * ESLint rule: no-cross-domain-imports
 *
 * Warns when a component imports from more than `maxDomains` distinct
 * `$components/<folder>/` domains. Domains listed in `excludeDomains` are not
 * counted (defaults to ["shared"], since shared/ is designed to be imported by
 * any component).
 *
 * Example config:
 *   "no-cross-domain-imports/no-cross-domain-imports": ["error", { maxDomains: 2 }]
 *   "no-cross-domain-imports/no-cross-domain-imports": ["error", { maxDomains: 3, excludeDomains: ["shared", "editor"] }]
 */

import type { Rule } from "eslint";

const COMPONENT_IMPORT_RE = /^\$components\/([^/]+)\//;

const DEFAULT_EXCLUDE_DOMAINS = ["shared"];

type Options = {
	maxDomains?: number;
	excludeDomains?: string[];
};

function create(context: any) {
	const opts = context.options[0] as Options | undefined;
	const maxDomains: number = opts?.maxDomains ?? 2;
	const excludeDomains = new Set<string>(opts?.excludeDomains ?? DEFAULT_EXCLUDE_DOMAINS);
	const domainNodes = new Map<string, any>();

	return {
		ImportDeclaration(node: any) {
			const importPath: string = node.source.value;
			const match = importPath.match(COMPONENT_IMPORT_RE);
			if (match) {
				const folder = match[1] ?? "";
				if (folder && !excludeDomains.has(folder) && !domainNodes.has(folder)) {
					domainNodes.set(folder, node);
				}
			}
		},
		"Program:exit"() {
			if (domainNodes.size > maxDomains) {
				const domains = [...domainNodes.keys()].sort();
				const filePath: string = context.getFilename();
				const folderMatch = filePath.match(/\/components\/([^/]+)\//);
				const currentFolder = folderMatch?.[1] ?? "unknown";
				const hint =
					currentFolder === "shared"
						? `shared/ is a utilities tier and should not import from domain folders at all. ` +
							`Move this component to views/ (if it composes multiple domains) or to the domain it most closely belongs to.`
						: `To fix: either move this component to views/ (if it's a composition-level component), ` +
							`move it to the domain it most closely belongs to, ` +
							`or extract the cross-domain logic into a separate component.`;
				const message =
					`Imports from ${domainNodes.size} $components domains (${domains.join(", ")}); maximum is ${maxDomains}. ` +
					hint +
					` This file is in ${currentFolder}/.`;
				const firstNode = domainNodes.values().next().value;
				context.report({ node: firstNode, message });
			}
		},
	};
}

export const noCrossDomainImports: Rule.RuleModule = {
	meta: {
		type: "suggestion",
		schema: {
			type: "array",
			minItems: 0,
			maxItems: 1,
			items: [
				{
					type: "object",
					properties: {
						maxDomains: { type: "number", minimum: 1 },
						excludeDomains: { type: "array", items: { type: "string" } },
					},
					additionalProperties: false,
				},
			],
		},
	},
	create,
};
