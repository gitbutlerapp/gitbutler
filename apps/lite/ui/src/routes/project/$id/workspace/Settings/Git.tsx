import { useSuspenseQuery } from "@tanstack/react-query";
import { useState, type FC } from "react";
import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { useSaveGUISettings } from "#ui/api/mutations.ts";
import { Range } from "#ui/components/Range.tsx";
import { clampAutoFetch, defaultSettings, parseAutoFetch } from "#ui/settings.ts";
import { formatDuration } from "#ui/time.ts";
import styles from "./Git.module.css";
import { Row, Section } from "./Section.tsx";

/**
 * The slider's stops, the first turning auto-fetch off. Each is stored as the string `ms`
 * parses, the shape the setting has always had.
 */
const autoFetchStops = [
	{ setting: "off", label: "Off" },
	{ setting: "5 min", label: "5min" },
	{ setting: "15 min", label: "15min" },
	{ setting: "30 min", label: "30min" },
	{ setting: "1 h", label: "1h" },
	{ setting: "2 h", label: "2h" },
];

const stopMs = autoFetchStops.map((stop) => parseAutoFetch(stop.setting));

const stopSetting = (index: number): string =>
	autoFetchStops[index]?.setting ?? defaultSettings.autoFetchFrequency;

/** The stop nearest a stored value, so one typed into an older build still lands on the slider. */
const nearestStop = (setting: string): number => {
	const ms = parseAutoFetch(setting);
	if (Number.isNaN(ms)) return 0;
	const distances = stopMs.map((stop) => Math.abs(stop - ms));
	return distances.indexOf(Math.min(...distances.filter((distance) => !Number.isNaN(distance))));
};

export const Git: FC = () => {
	const { data: settings } = useSuspenseQuery(guiSettingsQueryOptions);
	const { mutate: saveGUISettings } = useSaveGUISettings();

	const [autofetch, setAutofetch] = useState(
		settings.autoFetchFrequency ?? defaultSettings.autoFetchFrequency,
	);
	const autofetchMs = parseAutoFetch(autofetch);

	return (
		<Section>
			<Row
				stacked
				label="Auto-fetch frequency"
				hint={
					Number.isNaN(autofetchMs)
						? "Doesn't check your remotes for new commits."
						: `Checks your remotes for new commits every ${formatDuration(clampAutoFetch(autofetchMs))}.`
				}
			>
				<Range
					aria-label="Auto-fetch frequency"
					className={styles.range}
					value={nearestStop(autofetch)}
					min={0}
					max={autoFetchStops.length - 1}
					step={1}
					marks={autoFetchStops.map((stop, index) => ({ value: index, label: stop.label }))}
					onValueChange={(index) => setAutofetch(stopSetting(index))}
					onValueCommitted={(index) => saveGUISettings({ autoFetchFrequency: stopSetting(index) })}
				/>
			</Row>
		</Section>
	);
};
