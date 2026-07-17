import { Dialog } from "@base-ui/react";
import type { FC } from "react";
import styles from "./Settings.module.css";
import { useSuspenseQuery } from "@tanstack/react-query";
import { guiSettingsQueryOptions, listEditorsQueryOptions } from "#ui/api/queries.ts";
import { useSaveGUISettings } from "#ui/api/mutations.ts";
import type { ThemeCollectionFilter } from "@pierre/theming";
import { themes } from "@pierre/theming/themes";
import type { ThemesType } from "@pierre/diffs/react";
import { displayName } from "#ui/syntax-highlighting.ts";
import { classes } from "#ui/components/classes.ts";
import { defaultSettings } from "#ui/settings.ts";

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

	const setTheme = (variant: keyof ThemesType, themeName: string): void => {
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
								<label htmlFor="theme-light" className={styles.label}>
									Syntax theme (light)
								</label>

								<select
									id="theme-light"
									value={
										settings.syntaxHighlighting?.light ?? defaultSettings.syntaxHighlighting.light
									}
									onChange={(evt) => setTheme("light", evt.currentTarget.value)}
								>
									{lightThemes.map((theme) => (
										<option key={theme.name} value={theme.name}>
											{theme.displayName}
										</option>
									))}
								</select>
							</div>

							<div className={styles.input}>
								<label htmlFor="theme-dark" className={styles.label}>
									Syntax theme (dark)
								</label>

								<select
									id="theme-dark"
									value={
										settings.syntaxHighlighting?.dark ?? defaultSettings.syntaxHighlighting.dark
									}
									onChange={(evt) => setTheme("dark", evt.currentTarget.value)}
								>
									{darkThemes.map((theme) => (
										<option key={theme.name} value={theme.name}>
											{theme.displayName}
										</option>
									))}
								</select>
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
										evt.key === "Escape" &&
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
										evt.key === "Escape" &&
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
										evt.key === "Escape" &&
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
