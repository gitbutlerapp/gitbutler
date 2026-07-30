import { Dialog } from "@base-ui/react";
import { useDeferredValue, useState, type FC } from "react";
import styles from "./Settings.module.css";
import { useSuspenseQuery } from "@tanstack/react-query";
import { guiSettingsQueryOptions, listEditorsQueryOptions } from "#ui/api/queries.ts";
import { useSaveGUISettings } from "#ui/api/mutations.ts";
import type { ThemeCollectionFilter } from "@pierre/theming";
import { themes } from "@pierre/theming/themes";
import type { ThemesType } from "@pierre/diffs/react";
import { displayName } from "#ui/syntax-highlighting.ts";
import { classes } from "#ui/components/classes.ts";
import { clampAutoFetch as clampAutofetch, defaultSettings } from "#ui/settings.ts";
import * as ms from "ms";
import { formatDuration } from "#ui/time.ts";
import { ToggleGroup, Toggle } from "@base-ui/react";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";

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

type Props = {
	open: boolean;
	onOpenChange: (open: boolean) => void;
};

export const Settings: FC<Props> = ({ open, onOpenChange }) => {
	const { data: editors } = useSuspenseQuery(listEditorsQueryOptions);
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

	const [autofetch, setAutofetch] = useState(
		settings.autoFetchFrequency ?? defaultSettings.autoFetchFrequency,
	);
	const deferredAutofetch = useDeferredValue(autofetch);

	// Throws on empty and large strings.
	let parsedAutofetch: number;
	try {
		parsedAutofetch = ms.parse(deferredAutofetch);
	} catch {
		parsedAutofetch = Number.NaN;
	}

	const isValidAutofetch = !Number.isNaN(parsedAutofetch);

	return (
		<Dialog.Root open={open} onOpenChange={onOpenChange}>
			<Dialog.Portal>
				<Dialog.Backdrop className={styles.backdrop} />
				<Dialog.Viewport className={styles.viewport}>
					<Dialog.Popup aria-labelledby="settings-heading" className={styles.popup}>
						<h1
							id="settings-heading"
							className={classes("text-15", "text-semibold", styles.heading)}
						>
							Settings
						</h1>

						<div className={classes("text-13", styles.inputs)}>
							<div className={styles.input}>
								<label htmlFor="autofetch" className={styles.label}>
									Auto-fetch frequency
									<div className={classes("text-12", styles.inferredValue)}>
										{isValidAutofetch
											? formatDuration(clampAutofetch(parsedAutofetch))
											: "Disabled"}
									</div>
								</label>

								<input
									id="autofetch"
									type="text"
									value={autofetch}
									onChange={(evt) => setAutofetch(evt.currentTarget.value)}
									onBlur={(evt) =>
										saveGUISettings({
											autoFetchFrequency: evt.currentTarget.value,
										})
									}
									onKeyDown={(evt) =>
										(evt.key === "Enter" || evt.key === "Escape") &&
										saveGUISettings({
											autoFetchFrequency: evt.currentTarget.value,
										})
									}
								/>
							</div>

							<div className={styles.input}>
								<label htmlFor="editor" className={styles.label}>
									Default editor
								</label>

								<select
									id="editor"
									value={settings.editorId ?? ""}
									onChange={(evt) =>
										saveGUISettings({
											editorId: evt.currentTarget.value,
										})
									}
								>
									<option value="" disabled>
										Select an editor...
									</option>
									{editors.map((editor) => (
										<option key={editor.id} value={editor.id}>
											{editor.name}
										</option>
									))}
								</select>
							</div>

							<div className={styles.input}>
								{/* oxlint-disable-next-line jsx-a11y/label-has-associated-control */}
								<label id="theme" className={styles.label}>
									Theme
								</label>

								<ToggleGroup
									aria-labelledby="theme"
									value={[settings.theme ?? defaultSettings.theme]}
									onValueChange={([theme]) => {
										if (theme !== undefined) saveGUISettings({ theme });
									}}
									render={<ToggleGroupStyles />}
									className={styles.unstretchedValue}
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
							</div>

							<div className={styles.input}>
								<label htmlFor="syntax-theme-light" className={styles.label}>
									Syntax theme (light)
								</label>

								<select
									id="syntax-theme-light"
									value={
										settings.syntaxHighlighting?.light ?? defaultSettings.syntaxHighlighting.light
									}
									onChange={(evt) => setSyntaxTheme("light", evt.currentTarget.value)}
								>
									{lightThemes.map((theme) => (
										<option key={theme.name} value={theme.name}>
											{theme.displayName}
										</option>
									))}
								</select>
							</div>

							<div className={styles.input}>
								<label htmlFor="syntax-theme-dark" className={styles.label}>
									Syntax theme (dark)
								</label>

								<select
									id="syntax-theme-dark"
									value={
										settings.syntaxHighlighting?.dark ?? defaultSettings.syntaxHighlighting.dark
									}
									onChange={(evt) => setSyntaxTheme("dark", evt.currentTarget.value)}
								>
									{darkThemes.map((theme) => (
										<option key={theme.name} value={theme.name}>
											{theme.displayName}
										</option>
									))}
								</select>
							</div>

							<div className={styles.input}>
								{/* oxlint-disable-next-line jsx-a11y/label-has-associated-control */}
								<label id="unidiff" className={styles.label}>
									Diff files
								</label>

								<ToggleGroup
									aria-labelledby="unidiff"
									value={[String(settings.unidiff ?? defaultSettings.unidiff)]}
									onValueChange={([unidiff]) => {
										if (unidiff !== undefined) saveGUISettings({ unidiff: unidiff !== "false" });
									}}
									render={<ToggleGroupStyles />}
									className={styles.unstretchedValue}
								>
									<Toggle render={<ToggleStyles />} value="true">
										All-in-one diff
									</Toggle>
									<Toggle render={<ToggleStyles />} value="false">
										Selected file only
									</Toggle>
								</ToggleGroup>
							</div>

							<div className={styles.input}>
								<label htmlFor="font-family" className={styles.label}>
									Diff font family
								</label>

								<input
									id="font-family"
									type="text"
									defaultValue={settings.diffFontFamily ?? defaultSettings.diffFontFamily}
									onBlur={(evt) =>
										saveGUISettings({
											diffFontFamily: evt.currentTarget.value,
										})
									}
									onKeyDown={(evt) =>
										(evt.key === "Enter" || evt.key === "Escape") &&
										saveGUISettings({
											diffFontFamily: evt.currentTarget.value,
										})
									}
								/>
							</div>

							<div className={styles.input}>
								<label htmlFor="font-size" className={styles.label}>
									Diff font size
								</label>

								<input
									id="font-size"
									type="number"
									min={1}
									max={32}
									defaultValue={settings.diffFontSize ?? defaultSettings.diffFontSize}
									onBlur={(evt) =>
										saveGUISettings({
											diffFontSize: clamp(Number(evt.currentTarget.value), 1, 32),
										})
									}
									onKeyDown={(evt) =>
										(evt.key === "Enter" || evt.key === "Escape") &&
										saveGUISettings({
											diffFontSize: clamp(Number(evt.currentTarget.value), 1, 32),
										})
									}
								/>
							</div>

							<div className={styles.input}>
								<label htmlFor="tab-size" className={styles.label}>
									Diff tab size
								</label>

								<input
									id="tab-size"
									type="number"
									min={1}
									max={8}
									defaultValue={settings.diffTabSize ?? defaultSettings.diffTabSize}
									onBlur={(evt) =>
										saveGUISettings({
											diffTabSize: clamp(Number(evt.currentTarget.value), 1, 8),
										})
									}
									onKeyDown={(evt) =>
										(evt.key === "Enter" || evt.key === "Escape") &&
										saveGUISettings({
											diffTabSize: clamp(Number(evt.currentTarget.value), 1, 8),
										})
									}
								/>
							</div>
						</div>
					</Dialog.Popup>
				</Dialog.Viewport>
			</Dialog.Portal>
		</Dialog.Root>
	);
};
