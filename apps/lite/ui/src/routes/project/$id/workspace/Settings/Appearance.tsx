import { Toggle, ToggleGroup } from "@base-ui/react";
import { PatchDiff } from "@pierre/diffs/react";
import { useSuspenseQuery } from "@tanstack/react-query";
import type { FC } from "react";
import type { ThemeCollectionFilter } from "@pierre/theming";
import { themes } from "@pierre/theming/themes";
import type { ThemesType } from "@pierre/diffs/react";
import type { GUISettings } from "#electron/settings.ts";
import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { useSaveGUISettings } from "#ui/api/mutations.ts";
import { FieldControlStyles } from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { NumberField } from "#ui/components/NumberField.tsx";
import { Select } from "#ui/components/Select.tsx";
import { Switch } from "#ui/components/Switch.tsx";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { displayName } from "#ui/syntax-highlighting.ts";
import { defaultSettings } from "#ui/settings.ts";
import styles from "./Appearance.module.css";
import { Row, Section } from "./Section.tsx";

const getRenderableThemes = (filter?: ThemeCollectionFilter) =>
	themes
		.getThemes(filter)
		.map((theme) => ({
			name: theme.name,
			displayName: displayName(theme.name) ?? theme.displayName ?? theme.name,
		}))
		.toSorted((a, b) => a.displayName.localeCompare(b.displayName));

const lineDiffTypes = [
	{ value: "word-alt", label: "Words" },
	{ value: "word", label: "Words (whitespace-aware)" },
	{ value: "char", label: "Characters" },
	{ value: "none", label: "Off" },
] as const;

/** Enough of a diff to show a theme's ground, its gutter, and one change each way. */
const previewPatch = `diff --git a/hunk.ts b/hunk.ts
--- a/hunk.ts
+++ b/hunk.ts
@@ -1,3 +1,3 @@
 const hunk = parse("diff");
-if (hunk.ok) apply(hunk);
+if (hunk.ok) apply(hunk, { staged: true });
 return hunk;
`;

/**
 * A few lines of diff in one theme, with the select that chooses it underneath. The diff wears
 * the app's other diff settings too, so what it shows is what the diff view will.
 */
const SyntaxThemePreview: FC<{
	variant: keyof ThemesType;
	theme: string;
	themes: ReadonlyArray<{ name: string; displayName: string }>;
	settings: GUISettings;
	onChange: (theme: string) => void;
}> = (p) => (
	<div className={styles.preview}>
		<div
			className={styles.previewDiff}
			style={{
				"--diffs-font-family": p.settings.diffFontFamily ?? defaultSettings.diffFontFamily,
				"--diffs-font-size": `${p.settings.diffFontSize ?? defaultSettings.diffFontSize}px`,
				"--diffs-tab-size": `${p.settings.diffTabSize ?? defaultSettings.diffTabSize}`,
			}}
		>
			<PatchDiff
				patch={previewPatch}
				// The worker pool highlights with the pair the app has settled on; this shows the one
				// under consideration, so it highlights on its own.
				disableWorkerPool
				options={{
					theme: p.theme,
					themeType: p.variant,
					diffStyle: "unified",
					disableFileHeader: true,
					preferredHighlighter: "shiki-wasm",
					lineDiffType: p.settings.lineDiffType ?? defaultSettings.lineDiffType,
					disableBackground: !(p.settings.diffBackground ?? defaultSettings.diffBackground),
					unsafeCSS: `
						:host {
							font-variant-ligatures: ${
								(p.settings.diffLigatures ?? defaultSettings.diffLigatures) ? "normal" : "none"
							};
						}

						/* One hunk, and its header would say nothing the lines don't. */
						[data-separator] {
							display: none;
						}
					`,
				}}
			/>
		</div>

		<div className={styles.previewFooter}>
			<Select
				aria-label={`${p.variant === "light" ? "Light" : "Dark"} syntax theme`}
				searchable
				searchPlaceholder="Search themes..."
				nothingFound="No themes found"
				items={p.themes.map((theme) => ({ value: theme.name, label: theme.displayName }))}
				value={p.theme}
				onValueChange={(theme) => theme !== null && p.onChange(theme)}
			/>
		</div>
	</div>
);

export const Appearance: FC = () => {
	const { data: settings } = useSuspenseQuery(guiSettingsQueryOptions);
	const { mutate: saveGUISettings } = useSaveGUISettings();

	const setSyntaxTheme = (variant: keyof ThemesType, themeName: string): void => {
		saveGUISettings({
			syntaxHighlighting: {
				light: variant === "light" ? themeName : settings.syntaxHighlighting?.light,
				dark: variant === "dark" ? themeName : settings.syntaxHighlighting?.dark,
			},
		});
	};

	const lightThemes = getRenderableThemes({ colorScheme: "light" });
	const darkThemes = getRenderableThemes({ colorScheme: "dark" });

	return (
		<>
			<Section>
				<Row label="Theme" labelId="theme">
					<ToggleGroup
						aria-labelledby="theme"
						value={[settings.theme ?? defaultSettings.theme]}
						onValueChange={([theme]) => {
							if (theme !== undefined) saveGUISettings({ theme });
						}}
						render={<ToggleGroupStyles />}
					>
						<Toggle render={<ToggleStyles />} value="system">
							<Icon name="theme-system" />
							System
						</Toggle>
						<Toggle render={<ToggleStyles />} value="light">
							<Icon name="theme-light" />
							Light
						</Toggle>
						<Toggle render={<ToggleStyles />} value="dark">
							<Icon name="theme-dark" />
							Dark
						</Toggle>
					</ToggleGroup>
				</Row>

				<Row
					label="Hand cursor"
					labelId="hand-cursor"
					hint="Show a hand over buttons and links, like a web page."
				>
					<Switch
						size="large"
						aria-labelledby="hand-cursor"
						checked={settings.handCursor ?? defaultSettings.handCursor}
						onCheckedChange={(handCursor) => saveGUISettings({ handCursor })}
					/>
				</Row>
			</Section>

			<Section heading="Files">
				<Row
					label="File path first"
					labelId="path-first"
					hint="Put the folder before the file name in file lists."
				>
					<Switch
						size="large"
						aria-labelledby="path-first"
						checked={settings.pathFirst ?? defaultSettings.pathFirst}
						onCheckedChange={(pathFirst) => saveGUISettings({ pathFirst })}
					/>
				</Row>

				<Row
					label="File list right of the diff"
					labelId="files-panel-right"
					hint="So it doesn't sit next to the sidebar's file list."
				>
					<Switch
						size="large"
						aria-labelledby="files-panel-right"
						checked={settings.filesPanelRight ?? defaultSettings.filesPanelRight}
						onCheckedChange={(filesPanelRight) => saveGUISettings({ filesPanelRight })}
					/>
				</Row>
			</Section>

			<Section heading="Diff">
				<Row
					stacked
					label="Syntax theme"
					hint="Diffs switch between these when the app theme changes."
				>
					<div className={styles.themePreviews}>
						<SyntaxThemePreview
							variant="light"
							theme={settings.syntaxHighlighting?.light ?? defaultSettings.syntaxHighlighting.light}
							themes={lightThemes}
							settings={settings}
							onChange={(theme) => setSyntaxTheme("light", theme)}
						/>
						<SyntaxThemePreview
							variant="dark"
							theme={settings.syntaxHighlighting?.dark ?? defaultSettings.syntaxHighlighting.dark}
							themes={darkThemes}
							settings={settings}
							onChange={(theme) => setSyntaxTheme("dark", theme)}
						/>
					</div>
				</Row>

				<Row
					label="Files in the diff"
					labelId="unidiff"
					hint="All changed files at once, or just the one you picked."
				>
					<ToggleGroup
						aria-labelledby="unidiff"
						value={[String(settings.unidiff ?? defaultSettings.unidiff)]}
						onValueChange={([unidiff]) => {
							if (unidiff !== undefined) saveGUISettings({ unidiff: unidiff !== "false" });
						}}
						render={<ToggleGroupStyles />}
					>
						<Toggle render={<ToggleStyles />} value="true">
							All-in-one
						</Toggle>
						<Toggle render={<ToggleStyles />} value="false">
							Single file
						</Toggle>
					</ToggleGroup>
				</Row>

				{/* These three are the diff toolbar's own controls: one stored value each, so
				    changing either surface moves the other. */}
				<Row
					label="Layout"
					labelId="diff-style"
					hint="Split shows old and new side by side; unified stacks them."
				>
					<ToggleGroup
						aria-labelledby="diff-style"
						value={[settings.diffStyle ?? defaultSettings.diffStyle]}
						onValueChange={([diffStyle]) => {
							if (diffStyle !== undefined)
								saveGUISettings({ diffStyle: diffStyle as GUISettings["diffStyle"] });
						}}
						render={<ToggleGroupStyles />}
					>
						<Toggle render={<ToggleStyles />} value="split">
							Split
						</Toggle>
						<Toggle render={<ToggleStyles />} value="unified">
							Unified
						</Toggle>
					</ToggleGroup>
				</Row>

				<Row
					label="Soft wrap"
					labelId="soft-wrap"
					hint="Wrap long lines instead of scrolling them sideways."
				>
					<Switch
						size="large"
						aria-labelledby="soft-wrap"
						checked={(settings.diffOverflow ?? defaultSettings.diffOverflow) === "wrap"}
						onCheckedChange={(wrap) => saveGUISettings({ diffOverflow: wrap ? "wrap" : "scroll" })}
					/>
				</Row>

				<Row
					label="Diff backgrounds"
					labelId="diff-backgrounds"
					hint="Tint added and removed lines, rather than marking them by symbol alone."
				>
					<Switch
						size="large"
						aria-labelledby="diff-backgrounds"
						checked={settings.diffBackground ?? defaultSettings.diffBackground}
						onCheckedChange={(diffBackground) => saveGUISettings({ diffBackground })}
					/>
				</Row>

				<Row label="Font family" htmlFor="font-family">
					<FieldControlStyles
						id="font-family"
						type="text"
						className={styles.field}
						defaultValue={settings.diffFontFamily ?? defaultSettings.diffFontFamily}
						onBlur={(evt) => saveGUISettings({ diffFontFamily: evt.currentTarget.value })}
						onKeyDown={(evt) =>
							(evt.key === "Enter" || evt.key === "Escape") &&
							saveGUISettings({ diffFontFamily: evt.currentTarget.value })
						}
					/>
				</Row>

				<Row label="Font size">
					<NumberField
						aria-label="Font size"
						className={styles.number}
						value={settings.diffFontSize ?? defaultSettings.diffFontSize}
						min={1}
						max={32}
						step={1}
						onValueCommitted={(diffFontSize) =>
							diffFontSize !== null && saveGUISettings({ diffFontSize })
						}
					/>
				</Row>

				<Row label="Tab size">
					<NumberField
						aria-label="Tab size"
						className={styles.number}
						value={settings.diffTabSize ?? defaultSettings.diffTabSize}
						min={1}
						max={8}
						step={1}
						onValueCommitted={(diffTabSize) =>
							diffTabSize !== null && saveGUISettings({ diffTabSize })
						}
					/>
				</Row>

				<Row
					label="Font ligatures"
					labelId="ligatures"
					hint="Render combining glyphs such as → and !== if the font provides them."
				>
					<Switch
						size="large"
						aria-labelledby="ligatures"
						checked={settings.diffLigatures ?? defaultSettings.diffLigatures}
						onCheckedChange={(diffLigatures) => saveGUISettings({ diffLigatures })}
					/>
				</Row>

				<Row
					label="Highlight changes within a line"
					hint="How finely a changed line is compared against its counterpart."
				>
					<Select
						aria-label="Highlight changes within a line"
						className={styles.select}
						items={lineDiffTypes}
						value={settings.lineDiffType ?? defaultSettings.lineDiffType}
						onValueChange={(lineDiffType) =>
							lineDiffType !== null && saveGUISettings({ lineDiffType })
						}
					/>
				</Row>
			</Section>
		</>
	);
};
