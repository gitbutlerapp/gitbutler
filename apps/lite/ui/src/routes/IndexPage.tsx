import { signOut } from "#ui/hosted-session.ts";
import type { FC } from "react";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { EmptyState } from "@gitbutler/ui-react/EmptyState.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { AddProjectButton } from "#ui/components/AddProjectButton.tsx";
import { useAddLocalRepository } from "#ui/components/useAddLocalRepository.ts";
import { LiteTestId } from "#ui/testIds.ts";
import styles from "./IndexPage.module.css";

export const IndexPage: FC = () =>
	window.lite.hosted === true ? <NothingPublished /> : <Onboarding />;

const Onboarding: FC = () => {
	const { addLocalRepository, isPending } = useAddLocalRepository();

	return (
		<section className={styles.page} data-testid={LiteTestId.OnboardingPage}>
			<h1>Welcome to GitButler Next Nightly</h1>
			<p>Add a local Git repository to get started.</p>
			<AddProjectButton isPending={isPending} onClick={() => void addLocalRepository()} />
		</section>
	);
};

/** A hub opens its projects straight away, so this is only reached with nothing published. */
const NothingPublished: FC = () => (
	<section className={styles.page}>
		<EmptyState
			illustration="waving"
			title="Nothing published yet"
			description="Publish a branch from GitButler on any of your machines to see it here."
		>
			<Button variant="ghost" onClick={signOut}>
				Sign out
				<Icon name="logout" />
			</Button>
		</EmptyState>
	</section>
);
