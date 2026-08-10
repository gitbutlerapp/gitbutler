import { Toggle, ToggleGroup } from "@base-ui/react";
import { useSuspenseQuery } from "@tanstack/react-query";
import type { FC } from "react";
import type { ThemeCollectionFilter } from "@pierre/theming";
import { themes } from "@pierre/theming/themes";
import type { ThemesType } from "@pierre/diffs/react";
import type { GUISettings } from "#electron/settings.ts";
import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { useSaveGUISettings } from "#ui/api/mutations.ts";
import { Switch } from "#ui/components/Switch.tsx";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { displayName } from "#ui/syntax-highlighting.ts";
import { defaultSettings } from "#ui/settings.ts";
import { Row, Section } from "./Section.tsx";

const getRenderableThemes = (filter?: ThemeCollectionFilter) =>
	themes
		.getThemes(filter)
		.map((theme) => ({
			name: theme.name,
			displayName: displayName(theme.name) ?? theme.displayName ?? theme.name,
		}))
		.toSorted((a, b) => a.displayName.localeCompare(b.displayName));

const clamp = (value: number, min: number, max: number): number =>
	Math.min(Math.max(value, min), max);

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
							System
						</Toggle>
						<Toggle render={<ToggleStyles />} value="light">
							Light
						</Toggle>
						<Toggle render={<ToggleStyles />} value="dark">
							Dark
						</Toggle>
					</ToggleGroup>
				</Row>

				<Row label="Syntax theme (light)" htmlFor="syntax-theme-light">
					<select
						id="syntax-theme-light"
						value={settings.syntaxHighlighting?.light ?? defaultSettings.syntaxHighlighting.light}
						onChange={(evt) => setSyntaxTheme("light", evt.currentTarget.value)}
					>
						{lightThemes.map((theme) => (
							<option key={theme.name} value={theme.name}>
								{theme.displayName}
							</option>
						))}
					</select>
				</Row>

				<Row label="Syntax theme (dark)" htmlFor="syntax-theme-dark">
					<select
						id="syntax-theme-dark"
						value={settings.syntaxHighlighting?.dark ?? defaultSettings.syntaxHighlighting.dark}
						onChange={(evt) => setSyntaxTheme("dark", evt.currentTarget.value)}
					>
						{darkThemes.map((theme) => (
							<option key={theme.name} value={theme.name}>
								{theme.displayName}
							</option>
						))}
					</select>
				</Row>
			</Section>

			<Section heading="Files">
				<Row
					label="File path first"
					labelId="path-first"
					hint="Lead each row with the directory rather than the file name. The tree gives the directory a row of its own, so this is for the list."
				>
					<Switch
						aria-labelledby="path-first"
						checked={settings.pathFirst ?? defaultSettings.pathFirst}
						onCheckedChange={(pathFirst) => saveGUISettings({ pathFirst })}
					/>
				</Row>
			</Section>

			<Section heading="Diff">
				<Row
					label="Minimap"
					labelId="minimap"
					hint="A map of the diff down the right-hand edge, standing in for the scrollbar."
				>
					<Switch
						aria-labelledby="minimap"
						checked={settings.minimap ?? defaultSettings.minimap}
						onCheckedChange={(minimap) => saveGUISettings({ minimap })}
					/>
				</Row>

				<Row label="Diff files" labelId="unidiff">
					<ToggleGroup
						aria-labelledby="unidiff"
						value={[String(settings.unidiff ?? defaultSettings.unidiff)]}
						onValueChange={([unidiff]) => {
							if (unidiff !== undefined) saveGUISettings({ unidiff: unidiff !== "false" });
						}}
						render={<ToggleGroupStyles />}
					>
						<Toggle render={<ToggleStyles />} value="true">
							All-in-one diff
						</Toggle>
						<Toggle render={<ToggleStyles />} value="false">
							Selected file only
						</Toggle>
					</ToggleGroup>
				</Row>

				{/* These three are the diff toolbar's own controls: one stored value each, so
				    changing either surface moves the other. */}
				<Row label="Layout" labelId="diff-style" hint="Also on the diff toolbar.">
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
						aria-labelledby="diff-backgrounds"
						checked={settings.diffBackground ?? defaultSettings.diffBackground}
						onCheckedChange={(diffBackground) => saveGUISettings({ diffBackground })}
					/>
				</Row>

				<Row label="Font family" htmlFor="font-family">
					<input
						id="font-family"
						type="text"
						defaultValue={settings.diffFontFamily ?? defaultSettings.diffFontFamily}
						onBlur={(evt) => saveGUISettings({ diffFontFamily: evt.currentTarget.value })}
						onKeyDown={(evt) =>
							(evt.key === "Enter" || evt.key === "Escape") &&
							saveGUISettings({ diffFontFamily: evt.currentTarget.value })
						}
					/>
				</Row>

				<Row label="Font size" htmlFor="font-size">
					<input
						id="font-size"
						type="number"
						min={1}
						max={32}
						defaultValue={settings.diffFontSize ?? defaultSettings.diffFontSize}
						onBlur={(evt) =>
							saveGUISettings({ diffFontSize: clamp(Number(evt.currentTarget.value), 1, 32) })
						}
						onKeyDown={(evt) =>
							(evt.key === "Enter" || evt.key === "Escape") &&
							saveGUISettings({ diffFontSize: clamp(Number(evt.currentTarget.value), 1, 32) })
						}
					/>
				</Row>

				<Row
					label="Font ligatures"
					labelId="ligatures"
					hint="Render combining glyphs such as → and !== if the font provides them."
				>
					<Switch
						aria-labelledby="ligatures"
						checked={settings.diffLigatures ?? defaultSettings.diffLigatures}
						onCheckedChange={(diffLigatures) => saveGUISettings({ diffLigatures })}
					/>
				</Row>

				<Row
					label="Highlight changes within a line"
					htmlFor="line-diff-type"
					hint="How finely a changed line is compared against its counterpart."
				>
					<select
						id="line-diff-type"
						value={settings.lineDiffType ?? defaultSettings.lineDiffType}
						onChange={(evt) =>
							saveGUISettings({
								lineDiffType: evt.currentTarget.value as GUISettings["lineDiffType"],
							})
						}
					>
						<option value="word-alt">Words</option>
						<option value="word">Words (whitespace-aware)</option>
						<option value="char">Characters</option>
						<option value="none">Off</option>
					</select>
				</Row>

				<Row label="Tab size" htmlFor="tab-size">
					<input
						id="tab-size"
						type="number"
						min={1}
						max={8}
						defaultValue={settings.diffTabSize ?? defaultSettings.diffTabSize}
						onBlur={(evt) =>
							saveGUISettings({ diffTabSize: clamp(Number(evt.currentTarget.value), 1, 8) })
						}
						onKeyDown={(evt) =>
							(evt.key === "Enter" || evt.key === "Escape") &&
							saveGUISettings({ diffTabSize: clamp(Number(evt.currentTarget.value), 1, 8) })
						}
					/>
				</Row>
			</Section>
		</>
	);
};
