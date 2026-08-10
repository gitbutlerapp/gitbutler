/**
 * @file Local, GUI-specific settings.
 *
 * Settings are versioned and validated in decrementing order from the latest. Versions are for
 * breaking changes and should be possible to discriminate. Migrations may be performed from any
 * older version sequentially to the latest.
 */

import { app } from "electron";
import { type Type, type } from "arktype";
import { readFile, writeFile } from "atomically";
import path from "node:path";

const guiSettingsV1 = type({
	version: "1",
	"autoFetchFrequency?": "string",
	"autoUpdate?": "boolean",
	"diffBackground?": "boolean",
	"diffFontFamily?": "string",
	"diffFontSize?": "number",
	"diffLigatures?": "boolean",
	"diffOverflow?": "'scroll' | 'wrap'",
	"diffStyle?": '"unified" | "split"',
	"diffTabSize?": "number",
	"editorId?": "string",
	"fileDisplayMode?": "'list' | 'tree'",
	"lineDiffType?": "'word-alt' | 'word' | 'char' | 'none'",
	"minimap?": "boolean",
	"pathFirst?": "boolean",
	"terminalId?": "string",
	"syntaxHighlighting?": {
		"light?": "string",
		"dark?": "string",
	},
	"theme?": "'light' | 'dark' | 'system'",
	"unidiff?": "boolean",
});

type LegacyGUISettings = typeof guiSettingsV1.infer;

const legacyGUISettings: Type<LegacyGUISettings> = type.or(guiSettingsV1);

export type GUISettings = typeof guiSettingsV1.infer;

const emptySettings: GUISettings = { version: 1 };

/** Validate a foreign config, throwing on failure. */
const validate: (cfg: unknown) => LegacyGUISettings = legacyGUISettings.assert;

/**
 * Migrate older versioned configs to the latest. Noop if config is already the latest version.
 */
const migrate = (cfg: LegacyGUISettings): GUISettings => {
	switch (cfg.version) {
		// oxlint-disable-next-line typescript/no-unnecessary-condition -- There'll be more versions soon.
		case 1:
			return cfg;
	}
};

// Lazy, depends on dynamic app name.
const cfgPath = () => path.join(app.getPath("userData"), "settings.json");

/** Read the stored config, potentially performing migrations or writing a new config. */
export const readSettings = async (): Promise<GUISettings> => {
	try {
		const raw: unknown = JSON.parse(await readFile(cfgPath(), "utf8"));
		const legacy = validate(raw);
		const cfg = migrate(legacy);

		// oxlint-disable-next-line typescript/no-unnecessary-condition -- There'll be more versions soon.
		if (cfg.version !== legacy.version) await writeSettings(cfg);

		return cfg;
	} catch (e) {
		// oxlint-disable-next-line no-console
		console.warn(e);

		return emptySettings;
	}
};

export const writeSettings = (cfg: GUISettings): Promise<void> =>
	writeFile(cfgPath(), JSON.stringify(cfg, null, "  "));
