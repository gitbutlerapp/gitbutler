import {
	createHighlighter,
	type BundledLanguage,
	type BundledTheme,
	type Highlighter,
	type ThemedToken,
} from "shiki";

let highlighter: Highlighter | undefined;
let initPromise: Promise<Highlighter> | undefined;
const changeCallbacks: Array<() => void> = [];

const LANGUAGES = [
	"c",
	"cpp",
	"css",
	"dockerfile",
	"elixir",
	"go",
	"hcl",
	"html",
	"java",
	"javascript",
	"jinja",
	"json",
	"jsonc",
	"jsx",
	"kotlin",
	"lisp",
	"lua",
	"markdown",
	"nix",
	"php",
	"powershell",
	"python",
	"proto",
	"ruby",
	"rust",
	"shellscript",
	"svelte",
	"swift",
	"toml",
	"tsx",
	"typescript",
	"vue",
	"wasm",
	"xml",
	"yaml",
] as const;

export const DEFAULT_LIGHT_THEME = "github-light";
export const DEFAULT_DARK_THEME = "github-dark";

export interface SyntaxThemeOption {
	value: string;
	label: string;
}

export const LIGHT_THEMES: SyntaxThemeOption[] = [
	{ value: "ayu-light", label: "Ayu Light" },
	{ value: "catppuccin-latte", label: "Catppuccin Latte" },
	{ value: "everforest-light", label: "Everforest Light" },
	{ value: "github-light", label: "GitHub Light" },
	{ value: "github-light-default", label: "GitHub Light Default" },
	{ value: "github-light-high-contrast", label: "GitHub Light High Contrast" },
	{ value: "gruvbox-light-hard", label: "Gruvbox Light Hard" },
	{ value: "gruvbox-light-medium", label: "Gruvbox Light Medium" },
	{ value: "gruvbox-light-soft", label: "Gruvbox Light Soft" },
	{ value: "horizon-bright", label: "Horizon Bright" },
	{ value: "kanagawa-lotus", label: "Kanagawa Lotus" },
	{ value: "light-plus", label: "Light+ (VS Code)" },
	{ value: "material-theme-lighter", label: "Material Lighter" },
	{ value: "min-light", label: "Min Light" },
	{ value: "night-owl-light", label: "Night Owl Light" },
	{ value: "one-light", label: "One Light" },
	{ value: "rose-pine-dawn", label: "Rosé Pine Dawn" },
	{ value: "slack-ochin", label: "Slack Ochin" },
	{ value: "snazzy-light", label: "Snazzy Light" },
	{ value: "solarized-light", label: "Solarized Light" },
	{ value: "vitesse-light", label: "Vitesse Light" },
];

export const DARK_THEMES: SyntaxThemeOption[] = [
	{ value: "andromeeda", label: "Andromeeda" },
	{ value: "aurora-x", label: "Aurora X" },
	{ value: "ayu-dark", label: "Ayu Dark" },
	{ value: "ayu-mirage", label: "Ayu Mirage" },
	{ value: "catppuccin-frappe", label: "Catppuccin Frappé" },
	{ value: "catppuccin-macchiato", label: "Catppuccin Macchiato" },
	{ value: "catppuccin-mocha", label: "Catppuccin Mocha" },
	{ value: "dark-plus", label: "Dark+ (VS Code)" },
	{ value: "dracula", label: "Dracula" },
	{ value: "dracula-soft", label: "Dracula Soft" },
	{ value: "everforest-dark", label: "Everforest Dark" },
	{ value: "github-dark", label: "GitHub Dark" },
	{ value: "github-dark-default", label: "GitHub Dark Default" },
	{ value: "github-dark-dimmed", label: "GitHub Dark Dimmed" },
	{ value: "github-dark-high-contrast", label: "GitHub Dark High Contrast" },
	{ value: "gruvbox-dark-hard", label: "Gruvbox Dark Hard" },
	{ value: "gruvbox-dark-medium", label: "Gruvbox Dark Medium" },
	{ value: "gruvbox-dark-soft", label: "Gruvbox Dark Soft" },
	{ value: "houston", label: "Houston" },
	{ value: "kanagawa-dragon", label: "Kanagawa Dragon" },
	{ value: "kanagawa-wave", label: "Kanagawa Wave" },
	{ value: "laserwave", label: "Laserwave" },
	{ value: "material-theme", label: "Material" },
	{ value: "material-theme-darker", label: "Material Darker" },
	{ value: "material-theme-ocean", label: "Material Ocean" },
	{ value: "material-theme-palenight", label: "Material Palenight" },
	{ value: "min-dark", label: "Min Dark" },
	{ value: "monokai", label: "Monokai" },
	{ value: "night-owl", label: "Night Owl" },
	{ value: "nord", label: "Nord" },
	{ value: "one-dark-pro", label: "One Dark Pro" },
	{ value: "plastic", label: "Plastic" },
	{ value: "poimandres", label: "Poimandres" },
	{ value: "red", label: "Red" },
	{ value: "rose-pine", label: "Rosé Pine" },
	{ value: "rose-pine-moon", label: "Rosé Pine Moon" },
	{ value: "slack-dark", label: "Slack Dark" },
	{ value: "solarized-dark", label: "Solarized Dark" },
	{ value: "synthwave-84", label: "Synthwave '84" },
	{ value: "tokyo-night", label: "Tokyo Night" },
	{ value: "vesper", label: "Vesper" },
	{ value: "vitesse-black", label: "Vitesse Black" },
	{ value: "vitesse-dark", label: "Vitesse Dark" },
];

/**
 * Languages whose grammars require full-file context (e.g. `<script>` blocks)
 * to highlight embedded JS/TS. When line-by-line tokenization produces a
 * single unhelpful token, we retry with TypeScript as a fallback.
 */
export const SFC_LANGUAGES = new Set(["svelte", "vue"]);

// Currently selected themes (can be changed by the user via settings).
let selectedLightTheme: string = DEFAULT_LIGHT_THEME;
let selectedDarkTheme: string = DEFAULT_DARK_THEME;

/**
 * Detect whether the app is currently in dark mode by checking the DOM.
 */
function isDarkMode(): boolean {
	if (typeof document === "undefined") return false;
	return document.documentElement.classList.contains("dark");
}

/**
 * Returns the shiki theme name matching the app's current appearance.
 */
export function currentTheme(): string {
	return isDarkMode() ? selectedDarkTheme : selectedLightTheme;
}

/**
 * Set the user's preferred syntax themes. Loads the themes into shiki
 * if needed, then clears caches and triggers re-highlighting.
 */
export async function setSyntaxThemes(light: string, dark: string): Promise<void> {
	if (light === selectedLightTheme && dark === selectedDarkTheme) return;

	const h = getHighlighter();

	if (h) {
		const loaded = h.getLoadedThemes();
		if (!loaded.includes(light)) {
			await h.loadTheme(light as BundledTheme);
		}
		if (!loaded.includes(dark)) {
			await h.loadTheme(dark as BundledTheme);
		}
	}

	selectedLightTheme = light;
	selectedDarkTheme = dark;
	// Defer so this doesn't fire synchronously inside a Svelte $effect,
	// which would cause state writes during effect processing and trigger
	// an infinite update cycle.
	queueMicrotask(() => notifyChange());
}

let observerSetUp = false;

/**
 * Watch for dark/light mode changes on <html> and fire change callbacks.
 */
function observeThemeChanges(): void {
	if (observerSetUp || typeof document === "undefined") return;
	observerSetUp = true;

	const observer = new MutationObserver((mutations) => {
		for (const mutation of mutations) {
			if (mutation.type === "attributes" && mutation.attributeName === "class") {
				notifyChange();
				return;
			}
		}
	});

	observer.observe(document.documentElement, {
		attributes: true,
		attributeFilter: ["class"],
	});
}

function notifyChange(): void {
	for (const cb of changeCallbacks) {
		cb();
	}
}

/**
 * Initialize the shiki highlighter singleton. Safe to call multiple times.
 * Returns a promise that resolves when the highlighter is ready.
 */
export async function initHighlighter(): Promise<Highlighter> {
	if (highlighter) return await Promise.resolve(highlighter);
	if (initPromise) return await initPromise;

	initPromise = createHighlighter({
		themes: [selectedLightTheme as BundledTheme, selectedDarkTheme as BundledTheme],
		langs: [...LANGUAGES],
	})
		.then((h) => {
			highlighter = h;
			observeThemeChanges();
			notifyChange();
			return h;
		})
		.catch((e) => {
			console.error("Failed to initialize shiki highlighter:", e);
			initPromise = undefined;
			throw e;
		});

	return await initPromise;
}

/**
 * Get the highlighter if it's ready, or undefined if still loading.
 * Kicks off initialization on first call.
 */
export function getHighlighter(): Highlighter | undefined {
	if (!highlighter && !initPromise) {
		initHighlighter();
	}
	return highlighter;
}

/**
 * Register a callback for when highlighting should be re-applied.
 * This fires when the highlighter first becomes ready, and also
 * whenever the app theme (light/dark) or syntax theme selection changes.
 *
 * If the highlighter is already ready, the callback fires asynchronously
 * so the caller can do an initial render with highlighting.
 *
 * Returns an unsubscribe function that removes the callback.
 */
export function onHighlighterChange(callback: () => void): () => void {
	changeCallbacks.push(callback);

	if (highlighter) {
		// Defer so the callback doesn't fire synchronously during
		// registration — firing synchronously inside a Svelte $effect
		// can create an infinite re-trigger loop.
		queueMicrotask(() => {
			if (changeCallbacks.includes(callback)) {
				callback();
			}
		});
	} else if (!initPromise) {
		initHighlighter();
	}

	return () => {
		const idx = changeCallbacks.indexOf(callback);
		if (idx !== -1) {
			changeCallbacks.splice(idx, 1);
		}
	};
}

// Keep the old name working for any callers
export const onHighlighterReady = onHighlighterChange;

/**
 * Map a file extension to a shiki language ID.
 */
export function langFromExtension(extension: string): string | undefined {
	switch (extension) {
		case "js":
		case "mjs":
		case "cjs":
			return "javascript";
		case "jsx":
			return "jsx";
		case "ts":
		case "mts":
		case "cts":
			return "typescript";
		case "tsx":
			return "tsx";
		case "jsonc":
		case "json5":
			return "jsonc";
		case "json":
		case "jsonl":
			return "json";
		case "css":
			return "css";
		case "html":
			return "html";
		case "xml":
			return "xml";
		case "wasm":
			return "wasm";
		case "c":
		case "h":
			return "c";
		case "cc":
		case "cpp":
		case "c++":
		case "hpp":
		case "h++":
		case "hxx":
			return "cpp";
		case "ex":
		case "exs":
			return "elixir";
		case "go":
			return "go";
		case "hcl":
		case "hcl2":
		case "nomad":
		case "tf":
		case "tfvars":
			return "hcl";
		case "java":
			return "java";
		case "j2":
		case "jinja":
		case "jinja2":
			return "jinja";
		case "kt":
		case "kts":
			return "kotlin";
		case "lisp":
		case "lsp":
		case "cl":
		case "el":
			return "lisp";
		case "lua":
			return "lua";
		case "php":
			return "php";
		case "py":
		case "python":
			return "python";
		case "proto":
			return "proto";
		case "md":
			return "markdown";
		case "nix":
			return "nix";
		case "rs":
			return "rust";
		case "rb":
			return "ruby";
		case "toml":
			return "toml";
		case "yml":
		case "yaml":
			return "yaml";
		case "svelte":
			return "svelte";
		case "sh":
		case "bash":
		case "zsh":
			return "shellscript";
		case "swift":
			return "swift";
		case "vue":
			return "vue";
		case "ahk":
			return "powershell";
		default:
			return undefined;
	}
}

/**
 * Map a filename to a shiki language ID.
 */
export function langFromFilename(filename: string): string | undefined {
	const basename = filename.split("/").pop() || "";
	const ext = basename.split(".").pop()?.toLowerCase();

	if (basename === "Dockerfile" || basename.startsWith("Dockerfile.") || ext === "dockerfile") {
		return "dockerfile";
	}

	if (!ext) return undefined;
	return langFromExtension(ext);
}

/**
 * Tokenize a single line of code using shiki. Returns themed tokens.
 * Returns undefined if the highlighter is not yet ready.
 */
export function tokenizeLine(code: string, lang: string | undefined): ThemedToken[] | undefined {
	const h = getHighlighter();
	if (!h || !lang) return undefined;

	try {
		const result = h.codeToTokens(code, {
			lang: lang as BundledLanguage,
			theme: currentTheme(),
		});
		return result.tokens[0];
	} catch {
		return undefined;
	}
}

/**
 * Tokenize a multi-line block of code. Returns themed tokens per line.
 * Returns undefined if the highlighter is not yet ready.
 */
export function tokenizeCode(code: string, lang: string | undefined): ThemedToken[][] | undefined {
	const h = getHighlighter();
	if (!h || !lang) return undefined;

	try {
		const result = h.codeToTokens(code, {
			lang: lang as BundledLanguage,
			theme: currentTheme(),
		});
		return result.tokens;
	} catch {
		return undefined;
	}
}
