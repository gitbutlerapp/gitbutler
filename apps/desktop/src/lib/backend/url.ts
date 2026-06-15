import { showToast } from "$lib/notifications/toasts";
import { InjectionToken } from "@gitbutler/core/context";
import type { IBackend } from "$lib/backend";

const SEPARATOR = "/";

export const URL_SERVICE = new InjectionToken<URLService>("URLService");

export default class URLService {
	constructor(private backend: IBackend) {}

	async openExternalUrl(href: string) {
		try {
			await this.backend.openExternalUrl(href);
		} catch (e) {
			if (typeof e === "string" || e instanceof String) {
				const message = `
                Failed to open link in external browser:

                ${href}
            `;
				showToast({ title: "External URL error", message, style: "danger" });
			}

			// Rethrowing for sentry and posthog
			throw e;
		}
	}
}

export interface EditorUriParams {
	schemeId: string;
	path: string[];
	searchParams?: Record<string, string>;
	line?: number;
	column?: number;
}

// Mirrors `is_vscode_or_compatible` in `crates/but-api/src/open/mod.rs`.
const VSCODE_COMPATIBLE_SCHEMES = new Set([
	"vscode",
	"vscode-insiders",
	"vscodium",
	"cursor",
	"windsurf",
	"trae",
	"antigravity-ide",
]);

export function getEditorUri(params: EditorUriParams): string {
	// Query parameters (e.g. `windowId=_blank`) are only understood by VS Code
	// and its forks. Zed treats everything after `zed://file` as a literal file
	// path, so a query would end up in the opened path.
	const searchParams = VSCODE_COMPATIBLE_SCHEMES.has(params.schemeId)
		? params.searchParams
		: undefined;
	const searchParamsString = new URLSearchParams(searchParams).toString();
	// Separator is always a forward slash for editor paths, even on Windows
	const pathString = params.path.join(SEPARATOR);

	let positionSuffix = "";
	if (params.line !== undefined) {
		positionSuffix += `:${params.line}`;
		// Column is only valid if line is present
		if (params.column !== undefined) {
			positionSuffix += `:${params.column}`;
		}
	}

	const searchSuffix = searchParamsString ? `?${searchParamsString}` : "";

	return `${params.schemeId}://file${pathString}${positionSuffix}${searchSuffix}`;
}
