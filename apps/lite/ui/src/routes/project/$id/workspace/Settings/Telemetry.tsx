import type { FC } from "react";
import type { AppSettings } from "@gitbutler/but-sdk";
import { useUpdateTelemetry } from "#ui/api/mutations.ts";
import { Switch } from "#ui/components/Switch.tsx";
import { Row } from "./Section.tsx";
import styles from "./Telemetry.module.css";

/** The one telemetry choice Lite acts on: it sends usage metrics and reports no errors. */
export const UsageMetricsRow: FC<{ settings: AppSettings }> = ({ settings }) => {
	const { mutate: updateTelemetry } = useUpdateTelemetry();

	return (
		<Row
			label="Usage metrics"
			labelId="usage-metrics"
			hint={
				<>
					Counts of what the app does. Never file contents or sensitive data.{" "}
					<a
						href="https://gitbutler.com/privacy"
						className={styles.link}
						onClick={(event) => {
							event.preventDefault();
							void window.lite.openInWebBrowser(event.currentTarget.href);
						}}
					>
						Privacy policy
					</a>
				</>
			}
		>
			<Switch
				aria-labelledby="usage-metrics"
				checked={settings.telemetry.appMetricsEnabled}
				onCheckedChange={(appMetricsEnabled) => updateTelemetry({ appMetricsEnabled })}
			/>
		</Row>
	);
};
