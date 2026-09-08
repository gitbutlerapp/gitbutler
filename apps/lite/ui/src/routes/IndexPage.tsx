import type { FC } from "react";
import { AddProjectButton } from "#ui/components/AddProjectButton.tsx";
import { useAddLocalRepository } from "#ui/components/useAddLocalRepository.ts";
import { LiteTestId } from "#ui/testIds.ts";
import styles from "./IndexPage.module.css";

export const IndexPage: FC = () => {
	const { addLocalRepository, isPending } = useAddLocalRepository();

	return (
		<section className={styles.page} data-testid={LiteTestId.OnboardingPage}>
			<h1>Welcome to GitButler Lite</h1>
			<p>Add a local Git repository to get started.</p>
			<AddProjectButton isPending={isPending} onClick={() => void addLocalRepository()} />
		</section>
	);
};
