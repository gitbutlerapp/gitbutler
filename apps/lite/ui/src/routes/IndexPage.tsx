import { useQuery } from "@tanstack/react-query";
import type { FC } from "react";
import { useUpdateOnboardingComplete } from "#ui/api/mutations.ts";
import { appSettingsQueryOptions } from "#ui/api/queries.ts";
import { AddProjectButton } from "#ui/components/AddProjectButton.tsx";
import { useAddLocalRepository } from "#ui/components/useAddLocalRepository.ts";
import { UsageMetricsRow } from "#ui/routes/project/$id/workspace/Settings/Telemetry.tsx";
import { LiteTestId } from "#ui/testIds.ts";
import styles from "./IndexPage.module.css";

export const IndexPage: FC = () => {
	const { addLocalRepository, isPending } = useAddLocalRepository();
	const { data: settings } = useQuery(appSettingsQueryOptions);
	const { mutate: updateOnboardingComplete } = useUpdateOnboardingComplete();
	// Shown until the first repository is added: that is the confirmation.
	const confirming = settings !== undefined && !settings.onboardingComplete;

	return (
		<section className={styles.page} data-testid={LiteTestId.OnboardingPage}>
			<h1>Welcome to GitButler Lite</h1>
			<p>Add a local Git repository to get started.</p>
			{confirming && (
				<div className={styles.consent}>
					<UsageMetricsRow settings={settings} />
				</div>
			)}
			<AddProjectButton
				isPending={isPending}
				onClick={() =>
					void addLocalRepository().then((opened) => {
						if (opened && confirming) updateOnboardingComplete(true);
					})
				}
			/>
		</section>
	);
};
