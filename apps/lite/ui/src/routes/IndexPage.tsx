import { LiteTestId } from "@gitbutler/ui/utils/testIds";
import type { FC } from "react";
import { AddProjectButton } from "#ui/components/AddProjectButton.tsx";
import { useAddLocalRepository } from "#ui/components/useAddLocalRepository.ts";
import styles from "./IndexPage.module.css";

export const IndexPage: FC = () => {
	const { addLocalRepository, isPending } = useAddLocalRepository();

	return (
		<section className={styles.page} data-testid={LiteTestId.OnboardingPage}>
			<h1>Welcome to GitButler Lite</h1>
			<p>Add a local Git repository to get started.</p>
			<AddProjectButton
				testId={LiteTestId.OnboardingAddLocalProjectButton}
				isPending={isPending}
				onClick={() => void addLocalRepository()}
			/>
		</section>
	);
};
